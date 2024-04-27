use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

const NUM_SERVER_CONTAINERS: usize = 3; // N
const TOTAL_SLOTS: usize = 512; // #slots
const VIRTUAL_SERVERS_PER_CONTAINER: usize = 9; // K

// Hash function for request mapping
fn hash_request(req_id: usize) -> usize {
    (req_id + 2 * req_id + 17) % TOTAL_SLOTS
}

// Hash function for virtual server mapping
fn hash_virtual_server(container_id: usize, vs_index: usize) -> usize {
    (container_id + vs_index + 2 * vs_index + 25) % TOTAL_SLOTS
}

// ServerContainer represents a single server container in the load balancer
#[derive(Clone)]
pub struct ServerContainer {
    pub id: usize,
    pub name: String,
}

// VirtualServer represents a virtual server in the consistent hash map
pub struct VirtualServer {
    pub server_container: Arc<ServerContainer>,
    pub slot: usize,
}

// ServerPool maintains a consistent hash map and manages server containers
pub struct ServerPool {
    servers: Vec<Arc<ServerContainer>>,
    hash_map: Arc<RwLock<BTreeMap<usize, Arc<VirtualServer>>>>,
    slots: usize,
}

impl ServerPool {
    // Create a new server pool with the specified number of server containers and slots
    pub fn new(num_containers: usize, num_slots: usize) -> ServerPool {
        ServerPool {
            servers: Vec::with_capacity(num_containers),
            hash_map: Arc::new(RwLock::new(BTreeMap::new())),
            slots: num_slots,
        }
    }

    // Initialize the server pool with server containers and virtual servers
    pub fn initialize(&mut self) {
        // Create server containers
        for i in 0..NUM_SERVER_CONTAINERS {
            self.servers.push(Arc::new(ServerContainer {
                id: i,
                name: format!("Server-{}", i),
            }));
        }

        // Create virtual servers for each server container
        {
            let mut hash_map = self.hash_map.write().unwrap();
            for (i, container) in self.servers.iter().enumerate() {
                for j in 0..VIRTUAL_SERVERS_PER_CONTAINER {
                    let mut slot = hash_virtual_server(i, j);

                    // Apply linear probing if there's a conflict
                    while hash_map.contains_key(&slot) {
                        slot = (slot + 1) % self.slots;
                    }

                    hash_map.insert(
                        slot,
                        Arc::new(VirtualServer {
                            server_container: container.clone(),
                            slot,
                        }),
                    );
                }
            }
        }
    }

    // Retrieve the server container for a given request ID based on consistent hashing
    pub fn get_server_container(&self, req_id: usize) -> Option<Arc<ServerContainer>> {
        let slot = hash_request(req_id);
        let hash_map = self.hash_map.read().unwrap();

        // Direct match
        if let Some(vs) = hash_map.get(&slot) {
            return Some(vs.server_container.clone());
        }

        // Linear probing to find the nearest slot with a virtual server
        for i in 1..TOTAL_SLOTS {
            let check_slot = (slot + i) % self.slots;
            if let Some(vs) = hash_map.get(&check_slot) {
                return Some(vs.server_container.clone());
            }
        }

        None
    }

    // Return the list of server containers managed by the pool
    pub fn server_containers(&self) -> Vec<Arc<ServerContainer>> {
        self.servers.clone()
    }

    // Return the list of virtual servers in the consistent hash map
    pub fn virtual_servers(&self) -> Vec<Arc<VirtualServer>> {
        let hash_map = self.hash_map.read().unwrap();
        let mut vs_list: Vec<Arc<VirtualServer>> = hash_map.values().cloned().collect();

        // Sort by slot for easy visualization and debugging
        vs_list.sort_by_key(|vs| vs.slot);

        vs_list
    }
}