use std::path::Path;

#[derive(Debug, Clone)]
pub struct CodeChunk {
    pub file_path: String,
    pub content: String,
    pub start_line: usize,
}

pub fn chunk_file(file_path: &str, content: &str) -> Vec<CodeChunk> {
    let path = Path::new(file_path);
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    // Dla plików niestructuralnych (JSON, MD, TXT) zwracamy całość, ale dzielimy na części jeśli za duże
    if ["json", "md", "txt", "html", "xml", "csv"].contains(&extension) {
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut start_line = 1;
        let mut current_line = 1;

        for line in content.lines() {
            // [FIX] Limit 6000 znaków (~1500 tokenów) aby zmieścić się w limicie 2048 tokenów modelu Nomic
            if current_chunk.len() + line.len() > 6000 {
                chunks.push(CodeChunk {
                    file_path: file_path.to_string(),
                    content: current_chunk.clone(),
                    start_line: start_line,
                });
                current_chunk.clear();
                start_line = current_line;
            }
            current_chunk.push_str(line);
            current_chunk.push('\n');
            current_line += 1;
        }

        if !current_chunk.is_empty() {
             chunks.push(CodeChunk {
                file_path: file_path.to_string(),
                content: current_chunk,
                start_line: start_line,
            });
        }
        return chunks;
    }

    // Dla kodu (JS, RS, PY, TS, JAVA) - Semantic Chunking
    let mut chunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut current_chunk = String::new();
    let mut chunk_start_line = 1;
    let mut in_block = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current_chunk.is_empty() {
                current_chunk.push_str(line);
                current_chunk.push('\n');
            }
            continue;
        }

        // Oblicz wcięcie (liczba spacji na początku)
        let indent = line.chars().take_while(|c| c.is_whitespace()).count();
        let is_closing_brace = trimmed.starts_with('}') || trimmed.starts_with("];") || trimmed.starts_with(");");

        // [GLUON TUNING] Ulepszona logika chunkowania:
        // 1. Hard Limit: 4000 znaków (~1000 tokenów) - bezpieczny margines dla Nomic
        // 2. Soft Limit: > 1500 znaków - szukamy dobrego momentu na cięcie (małe wcięcie, koniec bloku)
        // 3. Logic Break: Wcięcie 0 (top-level definition)
        
        let hard_limit_reached = current_chunk.len() > 4000;
        let soft_limit_reached = current_chunk.len() > 1500;
        
        // Dobry moment na cięcie: 
        // - Koniec funkcji/klasy (zamykająca klamra przy małym wcięciu)
        // - Nowa definicja na najwyższym poziomie
        let is_good_cut_point = (indent == 0 && !trimmed.starts_with("}")) || (is_closing_brace && indent <= 4);

        if hard_limit_reached || (soft_limit_reached && is_good_cut_point) {
            // Zapisz obecny chunk
            if !current_chunk.trim().is_empty() {
                chunks.push(CodeChunk {
                    file_path: file_path.to_string(),
                    content: current_chunk.trim_end().to_string(),
                    start_line: chunk_start_line,
                });
                current_chunk = String::new();
                
                chunk_start_line = i + 1;
            }
        }

        current_chunk.push_str(line);
        current_chunk.push('\n');
    }

    // Dodaj ostatni chunk
    if !current_chunk.trim().is_empty() {
        chunks.push(CodeChunk {
            file_path: file_path.to_string(),
            content: current_chunk.trim_end().to_string(),
            start_line: chunk_start_line,
        });
    }

    chunks
}