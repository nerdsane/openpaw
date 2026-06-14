//! corridor_embed — the deterministic core of the embedding capability
//! (ADR-006, Track D1).
//!
//! The *judgment* (semantic similarity) comes from an external embedding
//! model (mxbai-embed-large via Ollama in dev; a sidecar/hosted endpoint in
//! prod — resolved from config, same as web search). The *decisions* —
//! diversity gating, clustering, nearest-match — are the pure, deterministic
//! functions here, run inside the consuming WASM module. No floats leave a
//! reproducible path: the same vectors always yield the same verdict.
//!
//! Consumers:
//! - sample_endpoints (D3): `farthest_point_order` / `is_diverse` to gate
//!   world diversity before the corridor spends sessions.
//! - spawn_repairers + evidence_ingest (D2): `nearest` to collapse an
//!   authored node that restates a determined/present fact.
//! - grade_hindcast / synthesis (D4): `nearest` to match actuals to
//!   forecasts; `cluster_by_threshold` to find cross-world agreement.
//!
//! This crate is pure (serde_json only). The HTTP fetch to the embedding
//! endpoint lives in each consumer (a ~15-line ctx.http_call), and its
//! response is parsed by `parse_embeddings` here.

use serde_json::{json, Value};

/// Build the request body for an embedding endpoint. `{"model", "input": [...]}`
/// is accepted by both Ollama `/api/embed` and OpenAI `/v1/embeddings`, so the
/// same body works for the dev (local Ollama) and prod (hosted) endpoints —
/// only the endpoint URL and model id change, via config. The model id is
/// passed by the consumer and stamped on the stored vectors for reproducibility
/// (ADR-006: a vector is only comparable to others from the same model).
pub fn build_embed_request(model: &str, texts: &[String]) -> String {
    json!({ "model": model, "input": texts }).to_string()
}

/// Cosine similarity of two equal-length vectors. Returns 0.0 for a length
/// mismatch or a zero-norm vector (degenerate — treated as "unrelated").
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Cosine *distance* in [0, 2]: 0 = identical direction, 1 = orthogonal,
/// 2 = opposite. Diversity and clustering reason in distance.
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    1.0 - cosine(a, b)
}

/// The smallest pairwise distance in a set — the set's diversity floor.
/// Returns f32::MAX for fewer than two vectors (nothing to be close to).
pub fn min_pairwise_distance(vectors: &[Vec<f32>]) -> f32 {
    let mut min = f32::MAX;
    for i in 0..vectors.len() {
        for j in (i + 1)..vectors.len() {
            let d = cosine_distance(&vectors[i], &vectors[j]);
            if d < min {
                min = d;
            }
        }
    }
    min
}

/// Is `candidate` far enough (>= threshold distance) from every vector in
/// `existing`? The diversity gate's accept test. Empty `existing` → true
/// (the first world is always accepted).
pub fn is_diverse(candidate: &[f32], existing: &[Vec<f32>], threshold: f32) -> bool {
    existing
        .iter()
        .all(|e| cosine_distance(candidate, e) >= threshold)
}

/// Greedy farthest-point ordering: start from index 0, then repeatedly pick
/// the unpicked vector whose distance to the nearest already-picked vector
/// is largest. Deterministic (ties broken by lowest index). Returns the
/// order in which to *keep* worlds so a truncated prefix is maximally
/// spread — the portfolio sampler's selection order.
pub fn farthest_point_order(vectors: &[Vec<f32>]) -> Vec<usize> {
    let n = vectors.len();
    if n == 0 {
        return Vec::new();
    }
    let mut picked = vec![0usize];
    let mut remaining: Vec<usize> = (1..n).collect();
    while !remaining.is_empty() {
        // For each remaining, its distance to the nearest picked vector.
        let mut best_idx = 0usize;
        let mut best_min_dist = f32::MIN;
        for (ri, &cand) in remaining.iter().enumerate() {
            let nearest = picked
                .iter()
                .map(|&p| cosine_distance(&vectors[cand], &vectors[p]))
                .fold(f32::MAX, f32::min);
            // Strictly-greater keeps the lowest-index tiebreak deterministic.
            if nearest > best_min_dist {
                best_min_dist = nearest;
                best_idx = ri;
            }
        }
        picked.push(remaining.remove(best_idx));
    }
    picked
}

/// Single-linkage clustering by a cosine-distance threshold: two vectors are
/// in the same cluster if a chain of within-threshold links connects them.
/// Returns a cluster id per input index (ids are the smallest member index,
/// so the assignment is deterministic and order-stable). Powers the
/// synthesis panel: claims that cluster across endpoints are the shared core.
pub fn cluster_by_threshold(vectors: &[Vec<f32>], threshold: f32) -> Vec<usize> {
    let n = vectors.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], x: usize) -> usize {
        let mut r = x;
        while parent[r] != r {
            r = parent[r];
        }
        // Path-compress.
        let mut c = x;
        while parent[c] != r {
            let next = parent[c];
            parent[c] = r;
            c = next;
        }
        r
    }
    for i in 0..n {
        for j in (i + 1)..n {
            if cosine_distance(&vectors[i], &vectors[j]) < threshold {
                let ri = find(&mut parent, i);
                let rj = find(&mut parent, j);
                if ri != rj {
                    // Union toward the smaller root (stable ids).
                    if ri < rj {
                        parent[rj] = ri;
                    } else {
                        parent[ri] = rj;
                    }
                }
            }
        }
    }
    (0..n).map(|i| find(&mut parent, i)).collect()
}

/// The nearest vector in `candidates` to `query`, as (index, distance), or
/// None if `candidates` is empty. Used by reconcile (is this authored node a
/// known fact?) and hindcast matching (which forecast does this actual hit?).
pub fn nearest(query: &[f32], candidates: &[Vec<f32>]) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;
    for (i, c) in candidates.iter().enumerate() {
        let d = cosine_distance(query, c);
        match best {
            Some((_, bd)) if d >= bd => {}
            _ => best = Some((i, d)),
        }
    }
    best
}

/// Greedy diversity selection: walk `candidates` in order and keep each one
/// that is at least `threshold` distant from everything kept so far (and from
/// the fixed `kept` references — worlds already released past the gate in an
/// earlier round). Returns a keep/re-steer flag per candidate. Deterministic
/// and order-stable: the first candidate of a near-duplicate pair is kept, the
/// second re-steered. The diversity gate's accept/re-steer decision (D3).
pub fn select_diverse(kept: &[Vec<f32>], candidates: &[Vec<f32>], threshold: f32) -> Vec<bool> {
    let mut acc: Vec<Vec<f32>> = kept.to_vec();
    candidates
        .iter()
        .map(|c| {
            if is_diverse(c, &acc, threshold) {
                acc.push(c.clone());
                true
            } else {
                false
            }
        })
        .collect()
}

/// Parse an embedding-endpoint response into row-aligned vectors. Accepts the
/// Ollama `/api/embed` shape `{"embeddings": [[...], ...]}`, the single-vector
/// `{"embedding": [...]}`, and the OpenAI `{"data": [{"embedding": [...]}]}`
/// shape — so swapping local Ollama for a hosted endpoint needs no code
/// change, only the endpoint secret.
pub fn parse_embeddings(body: &str) -> Vec<Vec<f32>> {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    // Ollama batch: {"embeddings": [[...], ...]}
    if let Some(arr) = v.get("embeddings").and_then(|x| x.as_array()) {
        return arr.iter().filter_map(as_vec).collect();
    }
    // OpenAI: {"data": [{"embedding": [...]}, ...]}
    if let Some(arr) = v.get("data").and_then(|x| x.as_array()) {
        return arr
            .iter()
            .filter_map(|e| e.get("embedding").and_then(as_vec))
            .collect();
    }
    // Ollama single: {"embedding": [...]}
    if let Some(single) = v.get("embedding").and_then(as_vec) {
        return vec![single];
    }
    Vec::new()
}

fn as_vec(v: &Value) -> Option<Vec<f32>> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(v: &[f32]) -> Vec<f32> {
        v.to_vec()
    }

    #[test]
    fn cosine_identical_orthogonal_opposite() {
        let a = unit(&[1.0, 0.0, 0.0]);
        let b = unit(&[1.0, 0.0, 0.0]);
        let c = unit(&[0.0, 1.0, 0.0]);
        let d = unit(&[-1.0, 0.0, 0.0]);
        assert!((cosine(&a, &b) - 1.0).abs() < 1e-6);
        assert!(cosine(&a, &c).abs() < 1e-6);
        assert!((cosine(&a, &d) + 1.0).abs() < 1e-6);
        // Distances: identical 0, orthogonal 1, opposite 2.
        assert!(cosine_distance(&a, &b).abs() < 1e-6);
        assert!((cosine_distance(&a, &c) - 1.0).abs() < 1e-6);
        assert!((cosine_distance(&a, &d) - 2.0).abs() < 1e-6);
        // Magnitude doesn't matter (direction only).
        assert!((cosine(&a, &unit(&[5.0, 0.0, 0.0])) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_degenerate_inputs_are_unrelated_not_panics() {
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0); // length mismatch
        assert_eq!(cosine(&[], &[]), 0.0); // empty
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0); // zero norm
    }

    #[test]
    fn is_diverse_accepts_first_and_rejects_near_duplicates() {
        let existing = vec![unit(&[1.0, 0.0]), unit(&[0.0, 1.0])];
        assert!(is_diverse(&[1.0, 1.0], &[], 0.3)); // first world always in
        // [1,0.05] is ~identical to existing [1,0] → distance ~0 → not diverse.
        assert!(!is_diverse(&[1.0, 0.05], &existing, 0.3));
        // [-1,-1] is far from both → diverse.
        assert!(is_diverse(&[-1.0, -1.0], &existing, 0.3));
    }

    #[test]
    fn farthest_point_order_spreads_selection() {
        // Three tight near [1,0] and one far at [0,1]; after index 0 the
        // farthest ([0,1]) must be picked before the near-duplicates.
        let vs = vec![
            unit(&[1.0, 0.0]),   // 0 (start)
            unit(&[0.99, 0.01]), // 1 near 0
            unit(&[0.98, 0.02]), // 2 near 0
            unit(&[0.0, 1.0]),   // 3 far
        ];
        let order = farthest_point_order(&vs);
        assert_eq!(order[0], 0);
        assert_eq!(order[1], 3, "the far world is picked second");
        // Deterministic.
        assert_eq!(farthest_point_order(&vs), order);
        assert!(farthest_point_order(&[]).is_empty());
    }

    #[test]
    fn cluster_by_threshold_groups_near_and_splits_far() {
        let vs = vec![
            unit(&[1.0, 0.0]),   // 0
            unit(&[0.99, 0.02]), // 1 ~ 0
            unit(&[0.0, 1.0]),   // 2
            unit(&[0.02, 0.99]), // 3 ~ 2
            unit(&[-1.0, -1.0]), // 4 alone
        ];
        let c = cluster_by_threshold(&vs, 0.3);
        assert_eq!(c[0], c[1], "near vectors share a cluster");
        assert_eq!(c[2], c[3]);
        assert_ne!(c[0], c[2], "far vectors split");
        assert_ne!(c[4], c[0]);
        assert_ne!(c[4], c[2]);
        // Cluster ids are the smallest member index (stable).
        assert_eq!(c[0], 0);
        assert_eq!(c[2], 2);
    }

    #[test]
    fn select_diverse_keeps_first_of_a_duplicate_pair_and_re_steers_the_rest() {
        // [1,0] and [0,1] are far apart (keep both); [0.99,0.02] collapses onto
        // [1,0] (re-steer); [-1,-1] is far from all (keep).
        let cands = vec![
            unit(&[1.0, 0.0]),
            unit(&[0.0, 1.0]),
            unit(&[0.99, 0.02]),
            unit(&[-1.0, -1.0]),
        ];
        let keep = select_diverse(&[], &cands, 0.3);
        assert_eq!(keep, vec![true, true, false, true]);
        // Fixed references (already-released worlds) suppress a near-duplicate
        // candidate: [1,0.01] collapses onto the kept reference [1,0].
        let refs = vec![unit(&[1.0, 0.0])];
        let keep2 = select_diverse(&refs, &[unit(&[1.0, 0.01]), unit(&[0.0, 1.0])], 0.3);
        assert_eq!(keep2, vec![false, true]);
        // Deterministic + empty candidates is empty.
        assert_eq!(select_diverse(&[], &cands, 0.3), keep);
        assert!(select_diverse(&refs, &[], 0.3).is_empty());
    }

    #[test]
    fn nearest_finds_closest_with_low_index_tiebreak() {
        let cands = vec![unit(&[1.0, 0.0]), unit(&[0.0, 1.0]), unit(&[0.9, 0.1])];
        let (i, d) = nearest(&[1.0, 0.0], &cands).unwrap();
        assert_eq!(i, 0);
        assert!(d < 1e-6);
        assert!(nearest(&[1.0, 0.0], &[]).is_none());
    }

    #[test]
    fn build_embed_request_is_portable_across_ollama_and_openai() {
        let body = build_embed_request("mxbai-embed-large", &["a".to_string(), "b".to_string()]);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["model"], "mxbai-embed-large");
        assert_eq!(v["input"][0], "a");
        assert_eq!(v["input"][1], "b");
        // Round-trips back through our own parser shape contract: input is an
        // array even for a single text (consumers always batch).
        let one = build_embed_request("m", &["solo".to_string()]);
        assert_eq!(serde_json::from_str::<Value>(&one).unwrap()["input"][0], "solo");
    }

    #[test]
    fn parse_embeddings_handles_ollama_and_openai_shapes() {
        // Ollama batch
        let o = r#"{"embeddings": [[1.0, 2.0], [3.0, 4.0]]}"#;
        assert_eq!(parse_embeddings(o), vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        // Ollama single
        let s = r#"{"embedding": [5.0, 6.0]}"#;
        assert_eq!(parse_embeddings(s), vec![vec![5.0, 6.0]]);
        // OpenAI
        let p = r#"{"data": [{"embedding": [7.0, 8.0]}]}"#;
        assert_eq!(parse_embeddings(p), vec![vec![7.0, 8.0]]);
        // Garbage → empty, never panic.
        assert!(parse_embeddings("not json").is_empty());
        assert!(parse_embeddings("{}").is_empty());
    }
}
