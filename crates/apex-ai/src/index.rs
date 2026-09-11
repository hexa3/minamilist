use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct CodeChunk {
    pub path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub embedding: Option<Vec<f32>>,
}

#[derive(Clone, Debug)]
pub struct SearchHit<'a> {
    pub chunk: &'a CodeChunk,
    pub score: f32,
}

#[derive(Clone, Debug, Default)]
pub struct RepositoryIndex {
    chunks: Vec<CodeChunk>,
    terms: Vec<HashMap<String, f32>>,
}

impl RepositoryIndex {
    pub fn build(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref();
        let mut paths = Vec::new();
        collect_files(root, root, &mut paths)?;
        let mut chunks = Vec::new();
        for path in paths {
            let Ok(source) = fs::read_to_string(root.join(&path)) else {
                continue;
            };
            for (index, lines) in source.lines().collect::<Vec<_>>().chunks(80).enumerate() {
                chunks.push(CodeChunk {
                    path: path.clone(),
                    start_line: index * 80 + 1,
                    end_line: index * 80 + lines.len(),
                    text: lines.join("\n"),
                    embedding: None,
                });
            }
        }
        let terms = chunks
            .iter()
            .map(|chunk| term_vector(&chunk.text))
            .collect();
        Ok(Self { chunks, terms })
    }

    pub fn chunks(&self) -> &[CodeChunk] {
        &self.chunks
    }

    pub fn set_embeddings(&mut self, embeddings: Vec<Vec<f32>>) -> bool {
        if embeddings.len() != self.chunks.len() {
            return false;
        }
        for (chunk, embedding) in self.chunks.iter_mut().zip(embeddings) {
            chunk.embedding = Some(embedding);
        }
        true
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit<'_>> {
        let query_terms = term_vector(query);
        let mut hits: Vec<_> = self
            .chunks
            .iter()
            .zip(&self.terms)
            .filter_map(|(chunk, terms)| {
                let score = cosine_sparse(&query_terms, terms);
                (score > 0.0).then_some(SearchHit { chunk, score })
            })
            .collect();
        hits.sort_by(|left, right| right.score.total_cmp(&left.score));
        hits.truncate(limit);
        hits
    }
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        if kind.is_symlink()
            || matches!(
                name.to_str(),
                Some(".git" | "target" | "node_modules" | ".apex")
            )
        {
            continue;
        }
        if kind.is_dir() {
            collect_files(root, &entry.path(), files)?;
        } else if kind.is_file() && entry.metadata()?.len() <= 2_000_000 {
            files.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .unwrap_or(&entry.path())
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn term_vector(text: &str) -> HashMap<String, f32> {
    let mut terms = HashMap::new();
    for term in text
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|term| term.len() >= 2)
        .map(str::to_ascii_lowercase)
    {
        *terms.entry(term).or_insert(0.0) += 1.0;
    }
    terms
}

fn cosine_sparse(left: &HashMap<String, f32>, right: &HashMap<String, f32>) -> f32 {
    let keys: HashSet<_> = left.keys().chain(right.keys()).collect();
    let dot = keys
        .iter()
        .map(|key| left.get(*key).unwrap_or(&0.0) * right.get(*key).unwrap_or(&0.0))
        .sum::<f32>();
    let left_norm = left.values().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right
        .values()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_search_finds_ai_provider_source() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let index = RepositoryIndex::build(root).unwrap();
        let hits = index.search("privacy network provider", 3);
        assert!(!hits.is_empty());
    }
}
