//! Shared SQL fragment builders for the scanners.
//!
//! Every scanner ends its query with the same two variable pieces: an `IN (...)`
//! list sized by `--names`/`--names-file`, and a row cap. Both are built here so
//! no scanner interpolates a value into SQL text: the cap is BOUND like any
//! other parameter, and the `IN` list emits placeholders only.
//!
//! GAP-SG-185 / v1.2.4: keyset pagination walks `id > last` pages.
//! `query_map` still collects per page (rusqlite statement lifetime). Prefer
//! [`keyset_for_each`] on the production enqueue path so only one page of
//! candidate keys is retained; [`keyset_collect`] still builds a full `Vec`
//! and is for dry-run / tests that need the complete ordered list.

use crate::errors::AppError;

/// SQLite sentinel for "no upper bound" in a `LIMIT` clause.
///
/// A negative `LIMIT` expression means unlimited, which lets every scan keep a
/// fixed `LIMIT ?n` placeholder instead of appending the number to the SQL text
/// when a cap is present and nothing when it is absent.
const NO_LIMIT: i64 = -1;

/// Converts an optional scan cap into the value bound to `LIMIT ?n`.
///
/// `None` becomes [`NO_LIMIT`]. A cap larger than [`i64::MAX`] saturates rather
/// than wrapping, which on any real corpus is indistinguishable from unlimited.
pub(in crate::commands::enrich) fn limit_param(limit: Option<usize>) -> i64 {
    limit.map_or(NO_LIMIT, |n| i64::try_from(n).unwrap_or(i64::MAX))
}

/// Renders `LIMIT ?{index}` for the given 1-based placeholder index.
///
/// `index` MUST be the position the limit occupies in the parameter list, i.e.
/// one past the last preceding parameter.
pub(in crate::commands::enrich) fn limit_clause(index: usize) -> String {
    format!("LIMIT ?{index}")
}

/// Renders `?{start}, ?{start + 1}, ...` for a `count`-element `IN (...)` list.
pub(super) fn placeholder_list(start: usize, count: usize) -> String {
    (start..start + count)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// How many rows the next keyset page should request.
///
/// Honours the remaining global `--limit` budget while never exceeding `page_size`.
pub(in crate::commands::enrich) fn page_take(page_size: usize, remaining: Option<usize>) -> usize {
    let page = page_size.max(1);
    match remaining {
        Some(r) => r.min(page),
        None => page,
    }
}

/// Walk an id-ordered scan in keyset pages, collecting every value.
///
/// `fetch_page(after_id, page_limit)` MUST return rows ordered by ascending id,
/// each as `(id, value)`. The page is dropped after it is appended so only the
/// accumulated values (and the next page) stay resident — never two full pages
/// of SQL rows at once. Callers that need O(page) peak should use
/// [`keyset_for_each`] instead of holding the returned `Vec`.
pub(in crate::commands::enrich) fn keyset_collect<T, F>(
    global_limit: Option<usize>,
    page_size: usize,
    mut fetch_page: F,
) -> Result<Vec<T>, AppError>
where
    F: FnMut(i64, usize) -> Result<Vec<(i64, T)>, AppError>,
{
    let mut out = Vec::new();
    keyset_for_each(global_limit, page_size, &mut fetch_page, |page| {
        out.extend(page);
        Ok(())
    })?;
    Ok(out)
}

/// Walk an id-ordered scan in keyset pages, invoking `on_page` for each page.
///
/// Returns the total number of values delivered. Peak RSS of the walker itself
/// is O(page_size) plus whatever `on_page` retains.
pub(in crate::commands::enrich) fn keyset_for_each<T, F, G>(
    global_limit: Option<usize>,
    page_size: usize,
    fetch_page: &mut F,
    on_page: G,
) -> Result<usize, AppError>
where
    F: FnMut(i64, usize) -> Result<Vec<(i64, T)>, AppError>,
    G: FnMut(Vec<T>) -> Result<(), AppError>,
{
    keyset_for_each_selected(
        global_limit,
        page_size,
        &mut |after, want| {
            Ok(fetch_page(after, want)?
                .into_iter()
                .map(|(id, value)| (id, Some(value)))
                .collect())
        },
        on_page,
    )
}

/// Keyset walk where a fetched row may be rejected after the query returned it.
///
/// `fetch_page` yields `(id, Option<T>)`: `None` marks a row that was scanned
/// but does not qualify. The distinction matters because two different counts
/// drive the walk, and conflating them corrupts it:
///
/// * page fullness is judged on rows SCANNED, so a page thinned by rejections is
///   not mistaken for the end of the table;
/// * `global_limit` and the returned total count values DELIVERED, so `--limit N`
///   still means N usable items rather than N rows looked at.
///
/// This is what lets `entity-descriptions` stream. Its SQL predicate cannot
/// express the low-quality-description test (`force_redescribe`), so rejection
/// necessarily happens in Rust after the fetch — and with the plain
/// [`keyset_for_each`] the first thinned page would have ended the scan early,
/// silently skipping every entity behind it.
pub(in crate::commands::enrich) fn keyset_for_each_selected<T, F, G>(
    global_limit: Option<usize>,
    page_size: usize,
    fetch_page: &mut F,
    mut on_page: G,
) -> Result<usize, AppError>
where
    F: FnMut(i64, usize) -> Result<Vec<(i64, Option<T>)>, AppError>,
    G: FnMut(Vec<T>) -> Result<(), AppError>,
{
    let mut after_id: i64 = 0;
    let mut remaining = global_limit;
    let mut total = 0usize;
    loop {
        let want = page_take(page_size, remaining);
        if want == 0 {
            break;
        }
        let page = fetch_page(after_id, want)?;
        if page.is_empty() {
            break;
        }
        let scanned = page.len();
        after_id = page.last().map(|(id, _)| *id).unwrap_or(after_id);
        let values: Vec<T> = page.into_iter().filter_map(|(_, v)| v).collect();
        let delivered = values.len();
        total = total.saturating_add(delivered);
        if delivered > 0 {
            on_page(values)?;
        }
        if let Some(r) = remaining.as_mut() {
            *r = r.saturating_sub(delivered);
        }
        if scanned < want {
            break;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_take_honours_remaining_budget() {
        assert_eq!(page_take(512, None), 512);
        assert_eq!(page_take(512, Some(10)), 10);
        assert_eq!(page_take(512, Some(0)), 0);
        assert_eq!(page_take(0, None), 1);
    }

    #[test]
    fn keyset_collect_pages_until_empty() {
        let pages: Vec<Vec<(i64, String)>> = vec![
            vec![(1, "a".into()), (2, "b".into())],
            vec![(3, "c".into())],
            vec![],
        ];
        let mut idx = 0;
        let out = keyset_collect(None, 2, |after, want| {
            assert!(want <= 2);
            let page = pages.get(idx).cloned().unwrap_or_default();
            idx += 1;
            if let Some((id, _)) = page.first() {
                assert!(*id > after || after == 0);
            }
            Ok(page)
        })
        .unwrap();
        assert_eq!(out, vec!["a", "b", "c"]);
    }

    #[test]
    fn keyset_for_each_drops_pages_and_respects_limit() {
        let mut seen = Vec::new();
        let total = keyset_for_each(
            Some(3),
            2,
            &mut |after, want| {
                let start = after + 1;
                let page: Vec<(i64, i64)> = (start..start + want as i64)
                    .map(|id| (id, id * 10))
                    .collect();
                Ok(page)
            },
            |page| {
                seen.push(page);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(total, 3);
        assert_eq!(seen, vec![vec![10, 20], vec![30]]);
    }
}
