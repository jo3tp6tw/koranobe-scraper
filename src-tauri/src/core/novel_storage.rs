// Persist novel metadata + chapters to a user-selected folder.
//
// Layout:
//   <root>/<sanitized title>/info.json
//   <root>/<sanitized title>/chapters/NNN_<sanitized chapter title>.txt

use std::path::{Path, PathBuf};

use crate::core::error::{ScrapeError, ScrapeResult};
use crate::templates::{Chapter, ChapterContent, NovelInfo};

/// Sanitize a string for use as a filename / directory name on Windows.
/// Removes characters: < > : " / \ | ? * and control chars.
pub fn sanitize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => out.push('_'),
            c if (c as u32) < 0x20 => out.push('_'),
            c => out.push(c),
        }
    }
    let trimmed = out.trim().trim_end_matches('.').to_string();
    // Avoid empty names
    if trimmed.is_empty() {
        return "_".to_string();
    }
    // Cap length to avoid Windows MAX_PATH issues
    let mut s: String = trimmed.chars().take(80).collect();
    s = s.trim().trim_end_matches('.').to_string();
    if s.is_empty() {
        "_".to_string()
    } else {
        s
    }
}

pub struct NovelStorage {
    pub root: PathBuf,
}

impl NovelStorage {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// Folder for a given novel title (created on demand).
    pub fn novel_dir(&self, title: &str) -> PathBuf {
        self.root.join(sanitize_name(title))
    }

    /// Prepare folder structure and write info.json. Returns the novel folder.
    ///
    /// Layout produced (flat, no `chapters/` subdir):
    ///   <root>/<title>/info.json
    ///   <root>/<title>/0000_簡介.txt   (written by `save_intro`)
    ///   <root>/<title>/0001_<chapter title>.txt
    pub fn init_novel(&self, info: &NovelInfo) -> ScrapeResult<PathBuf> {
        let dir = self.novel_dir(&info.title);
        std::fs::create_dir_all(&dir)?;
        let info_path = dir.join("info.json");
        let json = serde_json::to_string_pretty(info)?;
        std::fs::write(&info_path, json)?;
        Ok(dir)
    }

    /// Write `0000_簡介.txt` with title / introduction / tags in a human-readable form.
    pub fn save_intro(&self, novel_dir: &Path, info: &NovelInfo) -> ScrapeResult<PathBuf> {
        let path = novel_dir.join("0000_簡介.txt");
        let tags = info.tags.join("、");
        let body = if info.author.is_empty() {
            format!(
                "標題：{}\n\n簡介：\n{}\n\n標籤：{}\n",
                info.title, info.introduction, tags
            )
        } else {
            format!(
                "標題：{}\n\n作者：{}\n\n簡介：\n{}\n\n標籤：{}\n",
                info.title, info.author, info.introduction, tags
            )
        };
        std::fs::write(&path, body)?;
        Ok(path)
    }

    /// Path for a chapter file (regardless of existence). Lives at the novel folder root.
    pub fn chapter_path(&self, novel_dir: &Path, chapter: &Chapter) -> PathBuf {
        let filename = format!(
            "{:04}_{}.txt",
            chapter.index,
            sanitize_name(&chapter.title)
        );
        novel_dir.join(filename)
    }

    /// Skip-aware: returns true if the chapter file already exists with non-empty contents.
    pub fn chapter_exists(&self, novel_dir: &Path, chapter: &Chapter) -> bool {
        let p = self.chapter_path(novel_dir, chapter);
        match std::fs::metadata(&p) {
            Ok(m) => m.len() > 0,
            Err(_) => false,
        }
    }

    /// Remove chapter file if it exists (e.g. after mis-saving an empty paid gate page).
    pub fn remove_chapter(&self, novel_dir: &Path, chapter: &Chapter) -> ScrapeResult<()> {
        let p = self.chapter_path(novel_dir, chapter);
        if p.exists() {
            std::fs::remove_file(&p)?;
        }
        Ok(())
    }

    /// Character count of an on-disk chapter file (0 if missing).
    pub fn chapter_char_count(&self, novel_dir: &Path, chapter: &Chapter) -> usize {
        let p = self.chapter_path(novel_dir, chapter);
        std::fs::read_to_string(&p)
            .map(|s| s.chars().count())
            .unwrap_or(0)
    }

    /// Write a chapter file as plain text (title on first line, blank line, then body).
    pub fn save_chapter(
        &self,
        novel_dir: &Path,
        chapter: &Chapter,
        content: &ChapterContent,
    ) -> ScrapeResult<PathBuf> {
        let path = self.chapter_path(novel_dir, chapter);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = format!("{}\n\n{}\n", content.title, content.content);
        std::fs::write(&path, body).map_err(ScrapeError::from)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_invalid() {
        assert_eq!(sanitize_name("a:b/c?d*e"), "a_b_c_d_e");
        assert_eq!(sanitize_name("  trim  "), "trim");
        assert_eq!(sanitize_name(""), "_");
        assert_eq!(sanitize_name("name."), "name");
    }

    #[test]
    fn save_chapter_writes_file() {
        let tmp = std::env::temp_dir().join(format!(
            "novel-storage-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        let s = NovelStorage::new(&tmp);
        let info = NovelInfo {
            title: "T1: TestNovel".into(),
            author: String::new(),
            introduction: "這是介紹".into(),
            tags: vec!["a".into(), "b".into()],
            chapters: vec![],
        };
        let dir = s.init_novel(&info).unwrap();
        assert!(dir.join("info.json").exists());
        // Flat layout: no chapters/ subdir.
        assert!(!dir.join("chapters").exists());

        let intro_path = s.save_intro(&dir, &info).unwrap();
        assert_eq!(intro_path.file_name().unwrap(), "0000_簡介.txt");
        let intro = std::fs::read_to_string(&intro_path).unwrap();
        assert!(intro.contains("標題：T1: TestNovel"));
        assert!(intro.contains("這是介紹"));
        assert!(intro.contains("a、b"));

        let ch = Chapter {
            id: "ep1".into(),
            title: "第1話".into(),
            url: "http://x".into(),
            index: 1,
            requires_login: false,
        };
        let cc = ChapterContent {
            title: "第1話".into(),
            content: "Hello\nWorld".into(),
        };
        let path = s.save_chapter(&dir, &ch, &cc).unwrap();
        assert!(path.exists());
        // Chapter file lives at the novel root, not under chapters/.
        assert_eq!(path.parent().unwrap(), dir);
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("0001_"));
        assert!(s.chapter_exists(&dir, &ch));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
