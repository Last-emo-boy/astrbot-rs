use std::collections::{BTreeMap, BTreeSet};

use crate::types::KnowledgeChunk;

#[derive(Clone, Debug, PartialEq)]
pub struct RankFusionHit {
    pub chunk: KnowledgeChunk,
    pub score: f32,
}

impl RankFusionHit {
    pub fn new(chunk: KnowledgeChunk, score: f32) -> Self {
        Self { chunk, score }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReciprocalRankFusion {
    pub k: f32,
}

impl Default for ReciprocalRankFusion {
    fn default() -> Self {
        Self { k: 60.0 }
    }
}

impl ReciprocalRankFusion {
    pub fn fuse(
        &self,
        dense_results: Vec<RankFusionHit>,
        sparse_results: Vec<RankFusionHit>,
        top_k: usize,
    ) -> Vec<RankFusionHit> {
        let mut chunks = BTreeMap::new();
        let mut dense_ranks = BTreeMap::new();
        let mut sparse_ranks = BTreeMap::new();
        let mut ids = BTreeSet::new();

        for (index, hit) in dense_results.into_iter().enumerate() {
            ids.insert(hit.chunk.chunk_id.clone());
            dense_ranks.insert(hit.chunk.chunk_id.clone(), index + 1);
            chunks
                .entry(hit.chunk.chunk_id.clone())
                .or_insert(hit.chunk);
        }
        for (index, hit) in sparse_results.into_iter().enumerate() {
            ids.insert(hit.chunk.chunk_id.clone());
            sparse_ranks.insert(hit.chunk.chunk_id.clone(), index + 1);
            chunks
                .entry(hit.chunk.chunk_id.clone())
                .or_insert(hit.chunk);
        }

        let mut fused = ids
            .into_iter()
            .filter_map(|chunk_id| {
                let mut score = 0.0;
                if let Some(rank) = dense_ranks.get(&chunk_id) {
                    score += 1.0 / (self.k + *rank as f32);
                }
                if let Some(rank) = sparse_ranks.get(&chunk_id) {
                    score += 1.0 / (self.k + *rank as f32);
                }
                chunks
                    .remove(&chunk_id)
                    .map(|chunk| RankFusionHit::new(chunk, score))
            })
            .collect::<Vec<_>>();
        fused.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.chunk.chunk_index.cmp(&right.chunk.chunk_index))
        });
        fused.truncate(top_k.max(1));
        fused
    }
}
