use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentConfig {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub system_prompt: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub public_description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
}

impl SubagentConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            persona_id: None,
            system_prompt: String::new(),
            public_description: String::new(),
            provider_id: None,
            tools: Some(Vec::new()),
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn with_persona_id(mut self, persona_id: impl Into<String>) -> Self {
        self.persona_id = non_empty_option(persona_id);
        self
    }

    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = system_prompt.into();
        self
    }

    pub fn with_public_description(mut self, public_description: impl Into<String>) -> Self {
        self.public_description = public_description.into();
        self
    }

    pub fn with_provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = non_empty_option(provider_id);
        self
    }

    pub fn with_tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tools = Some(normalize_tools(tools));
        self
    }

    pub fn with_all_tools(mut self) -> Self {
        self.tools = None;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentConfigSource {
    #[serde(default)]
    pub agents: Vec<SubagentConfig>,
}

impl SubagentConfigSource {
    pub fn new(agents: Vec<SubagentConfig>) -> Self {
        Self { agents }
    }

    pub fn enabled_agents(&self) -> Vec<SubagentConfig> {
        self.agents
            .iter()
            .filter(|agent| agent.enabled && !agent.name.trim().is_empty())
            .cloned()
            .map(|mut agent| {
                agent.name = agent.name.trim().to_string();
                agent.persona_id = trim_option(agent.persona_id);
                agent.provider_id = trim_option(agent.provider_id);
                agent.system_prompt = agent.system_prompt.trim().to_string();
                agent.public_description = agent.public_description.trim().to_string();
                agent.tools = agent.tools.map(normalize_tools);
                agent
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubagentPersonaProfile {
    pub persona_id: String,
    pub system_prompt: Option<String>,
    pub begin_dialogs: Vec<String>,
    pub tools: Option<Vec<String>>,
}

impl SubagentPersonaProfile {
    pub fn new(persona_id: impl Into<String>) -> Self {
        Self {
            persona_id: persona_id.into(),
            system_prompt: None,
            begin_dialogs: Vec::new(),
            tools: Some(Vec::new()),
        }
    }

    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = non_empty_option(system_prompt);
        self
    }

    pub fn with_begin_dialog(mut self, begin_dialog: impl Into<String>) -> Self {
        let begin_dialog = begin_dialog.into();
        if !begin_dialog.trim().is_empty() {
            self.begin_dialogs.push(begin_dialog);
        }
        self
    }

    pub fn with_tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tools = Some(normalize_tools(tools));
        self
    }

    pub fn with_all_tools(mut self) -> Self {
        self.tools = None;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSubagent {
    pub name: String,
    pub instructions: String,
    pub public_description: Option<String>,
    pub persona_id: Option<String>,
    pub provider_id: Option<String>,
    pub tools: Option<Vec<String>>,
    pub begin_dialogs: Vec<String>,
}

impl ResolvedSubagent {
    pub fn from_config(config: SubagentConfig, persona: Option<SubagentPersonaProfile>) -> Self {
        let instructions = persona
            .as_ref()
            .and_then(|persona| persona.system_prompt.clone())
            .filter(|prompt| !prompt.trim().is_empty())
            .unwrap_or(config.system_prompt);
        let public_description = if config.public_description.trim().is_empty() {
            instructions_summary(&instructions)
        } else {
            Some(config.public_description)
        };
        let tools = if let Some(persona) = persona.as_ref() {
            persona.tools.clone()
        } else {
            config.tools
        };
        let begin_dialogs = persona
            .map(|persona| persona.begin_dialogs)
            .unwrap_or_default();

        Self {
            name: config.name,
            instructions,
            public_description,
            persona_id: config.persona_id,
            provider_id: config.provider_id,
            tools,
            begin_dialogs,
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    (!value.trim().is_empty()).then(|| value.trim().to_string())
}

fn trim_option(value: Option<String>) -> Option<String> {
    value.and_then(|value| (!value.trim().is_empty()).then(|| value.trim().to_string()))
}

fn normalize_tools<I, S>(tools: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut tools = tools
        .into_iter()
        .map(Into::into)
        .map(|tool| tool.trim().to_string())
        .filter(|tool| !tool.is_empty())
        .collect::<Vec<_>>();
    tools.sort();
    tools.dedup();
    tools
}

fn instructions_summary(instructions: &str) -> Option<String> {
    let instructions = instructions.trim();
    if instructions.is_empty() {
        return None;
    }
    Some(instructions.chars().take(120).collect())
}

#[cfg(test)]
mod tests {
    use super::{ResolvedSubagent, SubagentConfig, SubagentConfigSource, SubagentPersonaProfile};

    #[test]
    fn config_source_filters_disabled_and_blank_agents() {
        let source = SubagentConfigSource::new(vec![
            SubagentConfig::new(" analyst ").with_tools([" search ", "", "search"]),
            SubagentConfig::new("").with_system_prompt("skip"),
            SubagentConfig::new("disabled").disabled(),
        ]);

        let enabled = source.enabled_agents();

        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "analyst");
        assert_eq!(
            enabled[0].tools.as_deref(),
            Some(&["search".to_string()][..])
        );
    }

    #[test]
    fn resolved_subagent_prefers_persona_prompt_and_tools() {
        let resolved = ResolvedSubagent::from_config(
            SubagentConfig::new("analyst")
                .with_persona_id("persona-a")
                .with_system_prompt("inline prompt")
                .with_tools(["inline-tool"]),
            Some(
                SubagentPersonaProfile::new("persona-a")
                    .with_system_prompt("persona prompt")
                    .with_tools(["persona-tool"])
                    .with_begin_dialog("hello"),
            ),
        );

        assert_eq!(resolved.instructions, "persona prompt");
        assert_eq!(
            resolved.tools.as_deref(),
            Some(&["persona-tool".to_string()][..])
        );
        assert_eq!(resolved.begin_dialogs, ["hello"]);
        assert_eq!(
            resolved.public_description.as_deref(),
            Some("persona prompt")
        );
    }
}
