use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use nanorand::{Rng, WyRand};
use crate::config::SingleServer;

const NUM_SERVER_CONTAINERS: usize = 3; // N
const TOTAL_SLOTS: usize = 512; // #slots
const VIRTUAL_SERVERS_PER_CONTAINER: usize = 9; // K

// Hash function for request mapping
fn hash_request(req_id: usize, total_slots: usize) -> usize {
    (req_id + 2 * req_id + 17) % total_slots
}

// Hash function for virtual server mapping
fn hash_virtual_server(container_id: usize, vs_index: usize, total_slots: usize) -> usize {
    ((37 * container_id * vs_index) ^ (container_id | vs_index)) % total_slots
}


// VirtualServer represents a virtual server in the consistent hash map
#[derive(Clone, Debug)]
pub struct VirtualServer {
    pub server_container: SingleServer,
    pub slot: usize,
}

// ServerPool maintains a consistent hash map and manages server containers
#[derive(Clone)]
pub struct ServerPool {
    servers: Vec<SingleServer>,
    hash_map: BTreeMap<usize, VirtualServer>,
    num_containers: usize,
    pub rang_gen: WyRand,
}

impl ServerPool {
    // Create a new server pool with the specified number of server containers and slots
    pub fn new(num_containers: usize) -> ServerPool {
        ServerPool {
            servers: Vec::with_capacity(num_containers),
            hash_map: BTreeMap::new(),
            num_containers,
            rang_gen: nanorand::rand::WyRand::new_seed(32422312),
        }
    }

    // Initialize the server pool with server containers and virtual servers
    pub fn initialize(&mut self) {
        if !self.servers.is_empty() {
            // Create virtual servers for each server container
            let virtual_servers_per_container = TOTAL_SLOTS / self.servers.len();

            // virtual_servers_per_container is the number of times a single server will
            // be duplicated in our slot map, aka number of slots a mapping of virtual server
            // to physical server exist for each physical server
            for i in 0..virtual_servers_per_container {
                for container in &self.servers {
                    //  hash the server to get the slot
                    let mut slot = hash_virtual_server(container.id, i, TOTAL_SLOTS);

                    // Apply linear probing if there's a conflict
                    while self.hash_map.contains_key(&slot) {
                        slot = (slot + 1) % TOTAL_SLOTS;

                        if self.hash_map.len() >=TOTAL_SLOTS{
                            break;
                        }
                    }

                    self.hash_map.insert(
                        slot,
                        VirtualServer {
                            server_container: container.clone(),
                            slot,
                        },
                    );
                }
            }
        }
    }

    // Retrieve the server container for a given request ID based on consistent hashing
    pub fn get_server_container(&self, req_id: usize) -> Option<SingleServer> {
        let slot = hash_request(req_id, TOTAL_SLOTS);
        let hash_map = &self.hash_map;

        // Direct match
        if let Some(vs) = hash_map.get(&slot) {
            return Some(vs.server_container.clone());
        }

        // Linear probing to find the nearest slot with a virtual server
        for i in 1..TOTAL_SLOTS {
            let check_slot = (slot + i) % TOTAL_SLOTS;
            if let Some(vs) = hash_map.get(&check_slot) {
                return Some(vs.server_container.clone());
            }
        }

        None
    }
    pub fn add_server(&mut self, name: String, host: String, port: u16) {
        self.num_containers += 1;
        // Create server containers

        // generate random numbers
        self.servers.push(SingleServer {
            id: self.rang_gen.generate_range(100_000..999_999),
            name,
            host,
            port,
        });
        self.initialize();
    }

    // Return the list of server containers managed by the pool
    pub fn server_containers(&self) -> Vec<SingleServer> {
        self.servers.clone()
    }

    // Return the list of virtual servers in the consistent hash map
    pub fn virtual_servers(&self) -> Vec<VirtualServer> {
        let mut vs_list: Vec<VirtualServer> = self.hash_map.values().cloned().collect();

        // Sort by slot for easy visualization and debugging
        vs_list.sort_by_key(|vs| vs.slot);

        vs_list
    }
}


#[test]
fn test_out() {
    let mut containers = ServerPool::new(3);
    containers.initialize();
    containers.virtual_servers().iter().for_each(|c| println!("slot={} name={}", c.slot, &c.server_container.name));
}