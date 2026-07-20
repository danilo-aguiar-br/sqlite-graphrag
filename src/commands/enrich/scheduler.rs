//! Gate-first operation ordering and cooperative yield (GAP-CLI-PRIO-04/05, EC-09).
//!
//! Workload class: **mixed** — queue I/O is sequential; LLM fan-out is bounded
//! elsewhere. This module only decides *order* and *when to yield*.

use super::EnrichOperation;

/// Default yield cadence when neither flag nor XDG is set.
pub const DEFAULT_YIELD_EVERY_N_ITEMS: usize = 10;

/// Ops considered quality-gate (must run before entity-connect).
#[allow(dead_code)] // used by order_ops_gate_first + tests
pub fn is_gate_op(op: &EnrichOperation) -> bool {
    matches!(
        op,
        EnrichOperation::MemoryBindings | EnrichOperation::EntityDescriptions
    )
}

/// Expand `--ops-gate` into ordered gate operations.
pub fn gate_ops_order() -> Vec<EnrichOperation> {
    vec![
        EnrichOperation::MemoryBindings,
        EnrichOperation::EntityDescriptions,
    ]
}

/// Sort multi-op lists so gate ops run before entity-connect / bridges.
#[allow(dead_code)] // multi-op CLI expansion + tests
pub fn order_ops_gate_first(ops: &[EnrichOperation]) -> Vec<EnrichOperation> {
    let mut gate = Vec::with_capacity(ops.len());
    let mut rest = Vec::with_capacity(ops.len());
    for op in ops {
        if is_gate_op(op) {
            gate.push(op.clone());
        } else {
            rest.push(op.clone());
        }
    }
    // Stable preference: bindings before descriptions inside gate.
    gate.sort_by_key(|o| match o {
        EnrichOperation::MemoryBindings => 0,
        EnrichOperation::EntityDescriptions => 1,
        _ => 2,
    });
    // entity-connect last among rest
    rest.sort_by_key(|o| match o {
        EnrichOperation::EntityConnect | EnrichOperation::CrossDomainBridges => 10,
        EnrichOperation::ReEmbed => 5,
        _ => 1,
    });
    gate.extend(rest);
    gate
}

/// Resolve yield every N items: CLI flag > XDG > default.
pub fn resolve_yield_every_n(cli: Option<usize>) -> usize {
    if let Some(n) = cli {
        return n;
    }
    crate::runtime_config::resolve_usize(
        None,
        "enrich.yield_every_n_items",
        DEFAULT_YIELD_EVERY_N_ITEMS,
    )
}

/// Cooperative yield between batches (PRIO-05).
pub fn cooperative_yield() {
    std::thread::yield_now();
    // Tiny sleep reduces busy-spin when the OS scheduler ignores yield.
    std::thread::sleep(std::time::Duration::from_millis(1));
}

/// Whether a long-running EC drain should pause to let HOT ED work proceed.
pub fn should_preempt_for_hot_ed(has_hot_ed_pending: bool, current_is_ec: bool) -> bool {
    has_hot_ed_pending && current_is_ec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_first_puts_ed_before_ec() {
        let ops = vec![
            EnrichOperation::EntityConnect,
            EnrichOperation::EntityDescriptions,
            EnrichOperation::MemoryBindings,
        ];
        let ordered = order_ops_gate_first(&ops);
        assert_eq!(ordered[0], EnrichOperation::MemoryBindings);
        assert_eq!(ordered[1], EnrichOperation::EntityDescriptions);
        assert_eq!(ordered[2], EnrichOperation::EntityConnect);
    }

    #[test]
    fn preempt_only_when_ec_and_hot() {
        assert!(should_preempt_for_hot_ed(true, true));
        assert!(!should_preempt_for_hot_ed(true, false));
        assert!(!should_preempt_for_hot_ed(false, true));
    }
}
