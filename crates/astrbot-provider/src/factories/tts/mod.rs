mod commercial;
mod gemini;
mod gsv_selfhost;
mod gsvi;
mod minimax;
mod openai;
mod options;
mod volcengine;

pub(crate) use commercial::{
    build_azure_text_to_speech_provider, build_dashscope_text_to_speech_provider,
    build_edge_text_to_speech_provider, build_fishaudio_text_to_speech_provider,
    build_genie_text_to_speech_provider,
};
pub(crate) use gemini::build_gemini_text_to_speech_provider;
pub(crate) use gsv_selfhost::build_gsv_selfhost_text_to_speech_provider;
pub(crate) use gsvi::build_gsvi_text_to_speech_provider;
pub(crate) use minimax::build_minimax_text_to_speech_provider;
pub(crate) use openai::build_openai_text_to_speech_provider;
pub(crate) use volcengine::build_volcengine_text_to_speech_provider;
