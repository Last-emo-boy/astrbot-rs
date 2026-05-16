use crate::retrieval::KnowledgeRetrievalResult;

pub trait KnowledgeContextFormatter: Send + Sync {
    fn format_context(&self, results: &[KnowledgeRetrievalResult]) -> String;
}

#[derive(Clone, Debug)]
pub struct RetrievalContextFormatter {
    heading: String,
}

impl Default for RetrievalContextFormatter {
    fn default() -> Self {
        Self {
            heading: "以下是相关的知识库内容,请参考这些信息回答用户的问题:".to_string(),
        }
    }
}

impl RetrievalContextFormatter {
    pub fn new(heading: impl Into<String>) -> Self {
        Self {
            heading: heading.into(),
        }
    }
}

impl KnowledgeContextFormatter for RetrievalContextFormatter {
    fn format_context(&self, results: &[KnowledgeRetrievalResult]) -> String {
        let mut lines = vec![self.heading.clone(), String::new()];
        for (index, result) in results.iter().enumerate() {
            lines.push(format!("【知识 {}】", index + 1));
            if let (Some(kb_name), Some(doc_name)) = (&result.kb_name, &result.doc_name) {
                lines.push(format!("来源: {kb_name} / {doc_name}"));
            }
            lines.push(format!("内容: {}", result.content));
            lines.push(format!("相关度: {:.2}", result.score));
            lines.push(String::new());
        }
        lines.join("\n")
    }
}
