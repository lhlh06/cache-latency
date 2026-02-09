use hwlocality::cpu::binding::CpuBindingFlags;
use hwlocality::memory::binding::{MemoryBindingFlags, MemoryBindingPolicy};
use hwlocality::object::TopologyObject;
use hwlocality::object::types::ObjectType;
use hwlocality::topology::Topology;

pub struct SystemTopology {
    topology: Topology,
}

impl SystemTopology {
    pub fn new() -> Self {
        Self {
            topology: Topology::new().expect("Failed to load system topology"),
        }
    }

    /// Bind thread and NUMA
    ///
    /// # Parameters
    /// - `target_os_core`: target core PU
    /// - `numa_node_offset`:
    ///    - `0`: local NUMA node
    ///    - `1`: Next remote NUMA node
    pub fn bind(&self, target_os_core: usize, numa_node_offset: usize) {
        // find pu
        let pu = self
            .topology
            .objects_with_type(ObjectType::PU)
            .find(|obj| {
                obj.os_index()
                    .map(|idx| idx == target_os_core)
                    .unwrap_or(false)
            })
            .unwrap_or_else(|| panic!("Core ID {} not found in topology", target_os_core));

        // println!("Topology: Found PU (OS-ID={})", target_os_core);

        // bind CPU (Thread Binding)
        let cpuset = pu.cpuset().expect("PU object has no CPU set");

        // use CpuBindingFlags::THREAD to bind only current thread
        self.topology
            .bind_cpu(cpuset, CpuBindingFlags::THREAD)
            .expect("Failed to bind thread to CPU");

        // println!("  -> [Success] Thread pinned to Core #{}", target_os_core);

        // find Local NUMA Node
        // bottom-up find parent NUMA node
        let mut parent = pu.parent();
        let mut local_numa_obj: Option<&TopologyObject> = None;

        while let Some(obj) = parent {
            if obj.object_type() == ObjectType::NUMANode {
                local_numa_obj = Some(obj);
                break;
            }
            parent = obj.parent();
        }

        // NPS1: no parent NUMA
        // Assume only 1 NUMA
        let all_numa_nodes: Vec<_> = self
            .topology
            .objects_with_type(ObjectType::NUMANode)
            .collect();
        if all_numa_nodes.is_empty() {
            // println!("  -> [Warning] No NUMA nodes detected (UMA system). Memory binding skipped.");
            return;
        }

        // Single NUMA node
        if all_numa_nodes.len() == 1 {
            // println!(
            //     "  -> [Info] Single NUMA Node system detected (UMA). Skipping explicit memory binding (all memory is local)."
            // );
            return;
        }

        let local_node = local_numa_obj.unwrap_or_else(|| all_numa_nodes[0]);
        let local_os_index = local_node.os_index().expect("NUMA node has no OS index");

        // calulate target NUMA Node (Local vs Remote)
        let mut sorted_nodes: Vec<&TopologyObject> = all_numa_nodes.into_iter().collect();
        sorted_nodes.sort_by_key(|node| node.os_index().unwrap());

        let local_pos = sorted_nodes
            .iter()
            .position(|node| node.os_index().unwrap() == local_os_index)
            .unwrap();

        let target_pos = (local_pos + numa_node_offset) % sorted_nodes.len();
        let target_node = sorted_nodes[target_pos];
        let _target_os_index = target_node.os_index().unwrap();

        // Memory Binding
        let nodeset = target_node.nodeset().expect("NUMA node has no Node set");

        self.topology
            .bind_memory(
                nodeset,
                MemoryBindingPolicy::Bind,
                MemoryBindingFlags::STRICT,
            )
            .expect("Failed to bind memory");

        // let distance_desc = if numa_node_offset == 0 {
        //     "LOCAL"
        // } else {
        //     "REMOTE"
        // };
        // println!(
        //     "  -> [Success] Memory bound to {} NUMA Node #{} (OS-ID)",
        //     distance_desc, target_os_index
        // );
        //
        // if numa_node_offset > 0 && sorted_nodes.len() == 1 {
        //     println!("  -> [Warning] You requested Remote Memory, but only 1 NUMA node exists!");
        // }
    }
}
