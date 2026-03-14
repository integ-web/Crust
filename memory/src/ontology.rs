use petgraph::graph::DiGraph;
use petgraph::algo::tarjan_scc;
use std::collections::HashMap;

/// An Ontology Bridge that takes triples from Semantic Memory and performs
/// complex graph algorithms (like finding strongly connected components or simulating PageRank)
/// to find non-obvious connections during reasoning.
pub struct OntologyBridge {
    graph: DiGraph<String, String>,
    node_indices: HashMap<String, petgraph::graph::NodeIndex>,
}

impl OntologyBridge {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
        }
    }

    pub fn load_triplet(&mut self, subject: &str, predicate: &str, object: &str) {
        let s_idx = *self.node_indices.entry(subject.to_string()).or_insert_with(|| self.graph.add_node(subject.to_string()));
        let o_idx = *self.node_indices.entry(object.to_string()).or_insert_with(|| self.graph.add_node(object.to_string()));

        self.graph.add_edge(s_idx, o_idx, predicate.to_string());
    }

    /// Finds distinct communities of knowledge or non-obvious clusters using Tarjan's Strongly Connected Components
    pub fn find_communities(&self) -> Vec<Vec<String>> {
        let scc = tarjan_scc(&self.graph);
        let mut communities = Vec::new();

        for component in scc {
            let mut cluster = Vec::new();
            for idx in component {
                cluster.push(self.graph[idx].clone());
            }
            communities.push(cluster);
        }

        communities
    }
}
