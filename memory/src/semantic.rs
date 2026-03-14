use anyhow::Result;
use oxigraph::model::*;
use oxigraph::store::Store;
use oxigraph::sparql::{QueryResults, QuerySolution};
use tracing::info;

/// Semantic Memory: An RDF triple store used for logical inference and disambiguation.
pub struct SemanticMemory {
    store: Store,
}

impl SemanticMemory {
    pub fn new() -> Result<Self> {
        let store = Store::new()?;
        Ok(Self { store })
    }

    /// Reify a finding into an RDF Triple (Subject -> Predicate -> Object)
    pub fn insert_triplet(&self, subject: &str, predicate: &str, object: &str) -> Result<()> {
        let s = NamedNode::new(subject)?;
        let p = NamedNode::new(predicate)?;
        let o = Literal::new_simple_literal(object);

        self.store.insert(&Quad::new(s.clone(), p.clone(), o.clone(), GraphName::DefaultGraph))?;

        info!("Semantic Memory: Inserted triplet <{}> <{}> \"{}\"", subject, predicate, object);
        Ok(())
    }

    /// Queries the Semantic Memory using SPARQL
    pub fn query_sparql(&self, query: &str) -> Result<Vec<String>> {
        let results = self.store.query(query)?;

        let mut extracted = Vec::new();
        if let QueryResults::Solutions(solutions) = results {
            for solution in solutions {
                if let Ok(sol) = solution {
                    extracted.push(format!("{:?}", sol));
                }
            }
        }

        Ok(extracted)
    }
}
