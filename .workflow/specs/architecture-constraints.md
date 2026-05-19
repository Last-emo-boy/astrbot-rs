# Architecture Constraints

<spec-entry category="arch" keywords="rust-workspace,crate-boundaries,decoupling,astrbot" date="2026-05-15" source="E:/playground/Astrbot/astrbot/core/core_lifecycle.py">

### Crate Boundaries Must Preserve Runtime Separation

Rust 版应把 AstrBot 的核心理念拆成清晰 crate：`core` 定义事件、消息、配置、错误和生命周期端口；`platform` 实现消息平台适配；`provider` 实现 LLM/STT/TTS/Embedding/Rerank；`pipeline` 实现有序 Stage；`plugin` 实现插件注册和事件过滤；`dashboard` 只通过公开 API 观察和操作运行时。

核心 crate 不应依赖具体平台、具体 Provider、Dashboard 或插件实现。

</spec-entry>

<spec-entry category="decision" keywords="facade-roots,module-split,decoupling,runtime,provider,web,core,pipeline,maestro" date="2026-05-16" source="crates/astrbot-runtime/src/lib.rs:38; crates/astrbot-provider/src/registry.rs:821; crates/astrbot-web/src/lib.rs:133; crates/astrbot-core/src/message.rs:9; crates/astrbot-pipeline/src/context.rs:14; E:/Playground/Astrbot/astrbot/core/provider/manager.py:31">

### Large Crate Roots Stay Thin Facades

继续迁移 AstrBot provider/platform parity 前，应先拆掉当前 Rust 版的大文件耦合点。`lib.rs`、`registry.rs`、`message.rs`、`context.rs` 这类入口文件只应做 module 声明、公开 re-export 或单一边界组合，不应同时承载 config DTO、factory、manager、concrete adapter、policy port、HTTP DTO、转换逻辑和测试。

优先级按增长压力排序：`astrbot-runtime/src/lib.rs` 拆成 config/provider_config/policy_config/assembly/handle/ports/config_io；`astrbot-provider/src/registry.rs` 拆成 constants/capability/config/factories/options/registry/manager；`astrbot-provider/src/lib.rs` 拆成 chat/speech/tts/embedding/rerank/mock 门面；`astrbot-web/src/lib.rs` 拆成 DTO/routes/server/message_parts/error；`astrbot-core/src/message.rs` 拆成 component/chain/session/sink/result/provider_request/event；`astrbot-pipeline/src/context.rs` 拆成 context policy/ports/session/content_safety/provider_preference/result。

这与 AstrBot 的理念一致：ProviderManager 虽然统一管理 provider，但 chat、STT、TTS、embedding、rerank 是独立 capability bucket；Platform 也通过 register/manager 分开注册和装载。Rust 版应保留这些概念边界，并用模块和 trait re-export 保持编译期依赖清晰。后续大拆分必须先保持行为不变，包级测试通过后再继续新增 provider/platform 功能。

</spec-entry>

<spec-entry category="decision" keywords="gsvi-tts,text-to-speech,http-get,provider-registry" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/gsvi_tts_source.py">

### GSVI TTS Stays A Thin HTTP Adapter

AstrBot 的 `gsvi_tts_api` 是轻量 `ProviderType.TEXT_TO_SPEECH` adapter，只通过 `{api_base}/tts` GET query 传入 `text`、可选 `character` 和可选 `emotion`，成功响应直接写为 `.wav`。Rust 版应保持这个 provider 的薄边界，不引入额外模型生命周期、音频转换或 pipeline 语义。

`character` 可以从 `provider_options.character` 映射，也可复用通用 `TextToSpeechProviderConfig.voice` 作为 typed alias；`emotion` 保持 adapter-specific option。Runtime 只把它装入 `ProviderManager` 的 TTS bucket，pipeline 仍不直接消费 TTS，除非未来 voice flow 明确接入 `TextToSpeechProvider`。

</spec-entry>

<spec-entry category="decision" keywords="minimax-tts,text-to-speech,sse,provider-options,provider-registry" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/minimax_tts_api_source.py">

### MiniMax TTS Keeps Streaming Transport Inside The Provider

AstrBot 的 `minimax_tts_api` 是 `ProviderType.TEXT_TO_SPEECH` adapter，内部使用 SSE 传输接收 hex-encoded MP3 chunks，但对上层仍只暴露 `get_audio(text) -> path`。Rust 版应保持同样边界：`MiniMaxTextToSpeechProvider` 可以在 provider 内部收集 SSE chunks 并写 `.mp3`，但 `TextToSpeechProvider` trait 仍返回 `TextToSpeechResponse.audio_path`，不把 MiniMax 的 HTTP streaming 细节泄漏给 pipeline 或 runtime。

MiniMax 的 `minimax-group-id`、`minimax-langboost`、`minimax-is-timber-weight`、`minimax-timber-weight`、voice speed/volume/pitch/emotion/latex/normalization 等配置是 adapter-specific 字段，应继续通过 `TextToSpeechProviderConfig.provider_options` 和 runtime `provider_options` 映射。Runtime 只负责把它装入 `ProviderManager` 的 TTS bucket；pipeline 仍不直接消费 TTS，除非未来 voice flow 明确接入 `TextToSpeechProvider`。

</spec-entry>

<spec-entry category="decision" keywords="volcengine-tts,text-to-speech,provider-options,provider-registry" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/volcengine_tts.py">

### Volcengine TTS Uses Provider-Specific TTS Options

AstrBot 的 `volcengine_tts` 是独立 `ProviderType.TEXT_TO_SPEECH` adapter，配置字段包含 `appid`、`volcengine_cluster`、`volcengine_voice_type` 和 `volcengine_speed_ratio` 等 adapter-only key。Rust 版应保留 `TextToSpeechProvider` 能力边界，并通过 `TextToSpeechProviderConfig.provider_options` 承载这些字段，避免把 Volcengine 专属配置提升为所有 TTS provider 的公共字段。

Rust provider 必须保持 AstrBot-compatible 行为：provider type 为 `volcengine_tts`，请求头使用 `Authorization: Bearer; {api_key}`，请求体保留 app/user/audio/request 分组、`encoding = "mp3"`、`operation = "query"` 和 `frontend_type = "unitTson"`，响应从 `data` 字段解码 base64 MP3 并写入本地 `.mp3` 文件。Runtime 只把该 provider 映射进 `ProviderManager` 的 TTS bucket，pipeline 仍不直接消费 TTS，除非后续 voice flow 显式接入 `TextToSpeechProvider` trait。

</spec-entry>

<spec-entry category="decision" keywords="content-safety,strategy,keyword,pipeline-policy" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/pipeline/content_safety_check/stage.py">

### Content Safety Uses Strategy Traits

Rust 版应把 AstrBot `ContentSafetyCheckStage` 放在 policy stage 尾部、process 之前，默认顺序为 `wake -> whitelist -> session_status -> rate_limit -> content_safety -> process -> result_decorate -> respond`。内容安全只检查统一消息中的文本内容；图片、文件和响应侧检查留给后续明确 provider/result decoration 能力后再接入。

内容安全策略通过 `ContentSafetyStrategy` trait 注入 `PipelineContext`。当前只实现 keyword strategy；外部服务如百度 AIP 不应直接进入 pipeline crate，而应后续通过 strategy adapter 或 runtime/provider 边界接入。若 unsafe wake/at 消息需要用户提示，stage 先设置 result，让后续 `RespondStage` 发送；`ProcessStage` 必须跳过已有 result，避免插件或 provider 覆盖安全拦截提示。

</spec-entry>

<spec-entry category="decision" keywords="policy-stages,whitelist,session-status,rate-limit,pipeline-context" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/pipeline/whitelist_check/stage.py">

### Policy Stages Use Typed Context Ports

Rust 版应把 AstrBot `WhitelistCheckStage`、`SessionStatusCheckStage` 和 `RateLimitStage` 作为 wake 之后、process 之前的 policy stages，而不是塞进 EventBus 或 runtime。默认顺序为 `wake -> whitelist -> session_status -> rate_limit -> content_safety -> process -> result_decorate -> respond`。

白名单策略使用 `WhitelistPolicyConfig`，会话启停通过 `SessionStatusPort` trait 注入，限流使用 `RateLimitConfig` 和 stage 内部 fixed-window 计数器。Pipeline crate 不应依赖 AstrBot 的 `SessionServiceManager`、conversation manager 或具体持久化；这些后续由 runtime/dashboard/storage 层通过端口接入。

</spec-entry>

<spec-entry category="decision" keywords="wake-check,pipeline-policy,message-event,stage-order" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/pipeline/waking_check/stage.py">

### Wake Check Is The First Policy Stage

Rust 版应把 AstrBot `WakingCheckStage` 的唤醒边界放在默认 pipeline 最前面，而不是放进 EventBus、runtime 或 provider fallback。`WakeCheckStage` 通过 `PipelineContext::wake_check()` 读取 typed policy config；未命中的事件直接 stop，避免继续进入 plugin/provider/respond。

当前 Rust 阶段先覆盖低层可测行为：direct message 默认唤醒，group message 需要 wake prefix、bot mention、at-all 或 reply-to-bot 元数据；匹配 wake prefix 后从 `MessageChain` 中剥离 prefix。AstrBot 中 activated handler extras、permission reply、session plugin filtering 仍留给后续 ProcessStage 深化阶段。

</spec-entry>

<spec-entry category="decision" keywords="event-bus,pipeline,scheduler,message-flow" date="2026-05-15" source="E:/playground/Astrbot/astrbot/core/event_bus.py">

### EventBus Should Stay Thin

参考 AstrBot 的 `EventBus` 只消费平台事件队列、记录事件摘要、按配置找到对应 `PipelineScheduler` 并派发执行。Rust 版也应保持 EventBus 轻量，避免把权限、唤醒、插件、Provider 调用或发送逻辑塞进事件总线。

</spec-entry>

<spec-entry category="decision" keywords="plugin-system,registry,trait,extension-points" date="2026-05-15" source="E:/playground/Astrbot/astrbot/core/star/star_handler.py">

### Extension Points Use Traits And Registries

AstrBot 的 Star 机制使用 Handler Registry、EventType 和 Filter 组合扩展点。Rust 版应保留这个理念，但用 trait、typed metadata、registry 和依赖注入表达，减少动态导入和全局可变状态。

</spec-entry>

<spec-entry category="decision" keywords="plugin-sdk,sandbox,rust-native,capability,star,tool-execution" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/star/star_handler.py; E:/Playground/Astrbot/astrbot/core/star/star_manager.py; E:/Playground/Astrbot/astrbot/core/astr_agent_tool_exec.py:171; E:/Playground/Astrbot/astrbot/core/config/default.py:139">

### Plugin SDK And Sandbox Are Rust-Native Boundaries

Rust 版插件系统应学习 AstrBot Star 的 handler metadata、event type、filter、priority 和 plugin context 理念，但不必复制 Python 动态导入模型。`astrbot-plugin` 应逐步演进成自有 SDK：提供 typed `PluginContext`、event/command handler trait、插件 manifest、capability declaration、配置 schema、资源生命周期和测试 harness。宏或 builder API 可以作为易用层，但核心仍应是 trait/registry，方便静态检查和沙箱约束。

沙箱不应散落在 agent tool 执行逻辑里。参考 AstrBot 在 tool execution 前检查 sandbox capability 的实践，Rust 版应把沙箱抽成明确边界，例如 `SandboxRuntime`、`ToolCapability`、`PluginPermission`、`SandboxProfile` 和 per-session capability resolver。插件或工具声明需要的能力，runtime/pipeline 在调用前做 capability gate；具体执行可以后续接入进程隔离、WASM、受限文件系统、网络 allowlist 或外部 sandbox service。

Python 插件兼容可以作为后续 adapter，不应阻止 Rust-native SDK 先建立更强的类型、权限和生命周期模型。Dashboard 只能通过 SDK/manager 观察插件 manifest、状态和能力，不应直接接触插件实现或沙箱执行细节。

</spec-entry>

<spec-entry category="decision" keywords="provider-registry,provider-manager,default-provider" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/provider/manager.py">

### Provider Construction Stays Behind Registry And Manager

Rust 版应延续 AstrBot 的 `provider_cls_map`、`inst_map`、`default_provider_id` 分层理念：`ProviderRegistry` 负责 provider type 到 factory 的注册，`ProviderManager` 负责 configured provider ID 到实例的映射和默认选择。Pipeline、CLI、Dashboard 不应直接构造具体 Provider。

</spec-entry>

<spec-entry category="decision" keywords="provider-parity,openai-compatible,provider-aliases,registry" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/manager.py:356">

### OpenAI-Compatible Provider Parity Uses Registry Aliases First

AstrBot 当前把 `zhipu_chat_completion`、`groq_chat_completion`、`xai_chat_completion`、`aihubmix_chat_completion` 和 `openrouter_chat_completion` 动态导入为 OpenAI provider 路径的子类。Rust 版不应为这些 provider 复制独立 HTTP 实现；先通过 `ProviderRegistry` 注册 AstrBot-compatible provider type aliases，统一复用 `OpenAiCompatibleProvider`。

Provider alias 仍是配置和 UI 发现层面的真实类型，`ProviderManager` 按 configured provider ID 管理实例，pipeline 只看到最终 `ChatProvider` trait。OpenRouter 和 AIHubMix 的 AstrBot 默认 headers 可以在 registry factory 中注入；用户自定义 headers 保持由 config 覆盖或追加。

Groq 的 reasoning 字段清理、xAI native search、Anthropic/Gemini 非 OpenAI 协议、Embedding/Rerank/STT/TTS 等能力必须作为后续 provider capability 子任务实现，不能把未建模字段塞进当前 `ChatRequest`。

</spec-entry>

<spec-entry category="decision" keywords="anthropic,provider-parity,chat-provider,messages-api" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/anthropic_source.py:28">

### Anthropic Is A Native Provider Boundary

AstrBot 的 `ProviderAnthropic` 不是 OpenAI-compatible alias：它使用 Anthropic Messages API，把 system prompt 从消息列表中拆出，并从 response `content` blocks 中读取文本。Rust 版应提供独立 `AnthropicProvider`，但仍只能通过 `ProviderRegistry`/`ProviderManager` 进入 runtime，pipeline 继续只依赖 `ChatProvider` trait。

当前 Rust 实现先覆盖 text/system/context、data URL 图片、HTTP error mapping 和 response text parsing。Streaming、thinking config、tool use、remote image download 和 attachment-to-base64 转换都需要后续在明确 stream/tool/media 边界后实现；在此之前 provider 对 streaming 或非 data URL 图片应返回明确错误，而不是静默降级。

</spec-entry>

<spec-entry category="decision" keywords="gemini,provider-parity,chat-provider,generate-content" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/gemini_source.py:34">

### Gemini Is A Native GenerateContent Provider Boundary

AstrBot 的 `ProviderGoogleGenAI` 注册为 `googlegenai_chat_completion`，通过 Google GenAI `generate_content` 把 system instruction、user/model conversation contents、text parts 和 base64 image parts 发给 Gemini。Rust 版应提供独立 `GeminiProvider`，不能把 Gemini 塞进 OpenAI-compatible alias；入口仍必须是 `ProviderRegistry`/`ProviderManager`，runtime 只做 typed config 映射，pipeline 继续只依赖 `ChatProvider` trait。

当前 Rust 实现先覆盖非流式 text/system/context、data URL 图片、HTTP error mapping、response text parsing，以及 `SAFETY`、`PROHIBITED_CONTENT`、`SPII`、`BLOCKLIST`、`IMAGE_SAFETY` finish reason 的明确错误。Streaming、tool calls、native search、URL context、thinking config、安全设置细分、remote image download 和 attachment-to-base64 转换都需要后续在 stream/tool/media/config capability 边界明确后实现；在此之前 provider 对 streaming 或非 data URL 图片应返回明确错误。

</spec-entry>

<spec-entry category="decision" keywords="provider-capability,provider-registry,metadata,stt,tts,embedding,rerank" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/entities.py:27">

### Provider Capability Is Metadata Before Concrete Factories

AstrBot separates provider adapter identity from capability through `ProviderType`: chat completion, speech-to-text, text-to-speech, embedding, and rerank. Rust 版应保留这个分层，但不能在当前 chat-only pipeline 中提前塞入未建模的非 chat provider 行为。

`ProviderRegistry` 先引入 `ProviderCapability` 和 `ProviderAdapterMetadata`。Chat provider factory 注册会自动发布 `chat_completion` metadata；STT/TTS/Embedding/Rerank 分别通过自己的 trait、config、factory、manager bucket 和 runtime 映射演进。`ProviderManager::from_chat_configs()` 仍是 chat-only，遇到非 chat metadata 时必须明确报错，避免把 Embedding/Rerank/STT/TTS 配置误构造成 `ChatProvider`。

后续非 chat provider 应学习 AstrBot manager 的 capability buckets，但用 Rust 分离的 typed managers/ports 表达，不能把所有能力继续塞进 `ChatRequest` 或默认 pipeline。

</spec-entry>

<spec-entry category="decision" keywords="stt,tts,provider-capability,provider-manager,registry" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/provider.py:198">

### STT And TTS Provider Boundaries Stay Separate From Chat

AstrBot 的 `STTProvider` 单独定义 `get_text(audio_url)`，`TTSProvider` 单独定义 `get_audio(text)` 和 `support_stream()`，并由 `ProviderManager` 放入独立的 `stt_provider_insts` 与 `tts_provider_insts` capability buckets。Rust 版应保留这个分离边界，避免把语音识别或语音合成请求塞进 `ChatRequest` 或默认 LLM pipeline。

当前 Rust 实现先在 provider crate 内建立 `SpeechToTextRequest`、`SpeechToTextResponse`、`SpeechToTextProvider`、`TextToSpeechRequest`、`TextToSpeechResponse`、`TextToSpeechProvider`、mock STT/TTS provider、registry factory 和 `ProviderManager` STT/TTS routing。TTS streaming 先作为 provider capability query 暴露，不实现音频流队列。OpenAI TTS、Gemini TTS、OpenAI Whisper STT 和 Xinference STT 已作为第一批 concrete voice adapters 接入 provider crate；Azure/DashScope/Edge/Xinference TTS 等其他 STT/TTS adapters、runtime config、pipeline voice flow 和 dashboard 接入仍暂缓，等 voice provider runtime 边界稳定后再接入。

</spec-entry>

<spec-entry category="decision" keywords="openai-tts,provider-parity,text-to-speech,registry,audio-file" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/openai_tts_api_source.py:16">

### OpenAI TTS Writes Audio Behind The TTS Boundary

AstrBot 的 `openai_tts_api` 是 `ProviderType.TEXT_TO_SPEECH` 适配器，通过 OpenAI-compatible audio speech API 发送 `model`、`voice`、`response_format="wav"` 和 `input`，并把返回的音频字节写入临时 `.wav` 文件，再返回文件路径。Rust 版应保留这个 provider type，但实现为独立 `OpenAiTextToSpeechProvider`，通过 `TextToSpeechProvider` trait、`TextToSpeechProviderConfig` 和 `ProviderRegistry` 进入 manager。

当前 Rust 实现只在 provider crate 内提供 HTTP adapter、bearer auth、custom headers、错误解析和音频文件落盘。`TextToSpeechResponse` 继续返回 `audio_path`，runtime/pipeline/dashboard 不直接处理音频字节。后续若支持流式 TTS，应先扩展 TTS-specific streaming boundary，而不是复用 chat streaming。

</spec-entry>

<spec-entry category="decision" keywords="gemini-tts,provider-parity,text-to-speech,registry,wav" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/gemini_tts_source.py:15">

### Gemini TTS Writes PCM Inline Data As WAV

AstrBot 的 `gemini_tts` 是 `ProviderType.TEXT_TO_SPEECH` 适配器，通过 Gemini `generateContent` 请求音频响应，并把返回的 inline audio data 解码成 PCM 后写成 24 kHz mono 16-bit WAV 文件。Rust 版应保留这个 provider type，但实现为独立 `GeminiTextToSpeechProvider`，通过 `TextToSpeechProvider` trait、`TextToSpeechProviderConfig` 和 `ProviderRegistry` 进入 manager。

当前 Rust 实现只在 provider crate 内提供 REST adapter、API key header、custom headers、错误解析、base64 inline PCM 解码和 WAV 文件落盘。`TextToSpeechResponse` 继续返回 `audio_path`，runtime/pipeline/dashboard 不直接处理音频字节。Gemini TTS 与 Gemini chat/embedding 共享 provider 家族理念，但不能复用 `ChatRequest` 或 embedding 边界。

</spec-entry>

<spec-entry category="decision" keywords="openai-stt,whisper,provider-parity,speech-to-text,registry,media-conversion" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/whisper_api_source.py:19">

### OpenAI Whisper STT Stays Behind The STT Boundary

AstrBot 的 `openai_whisper_api` 是 `ProviderType.SPEECH_TO_TEXT` 适配器，通过 OpenAI-compatible audio transcription API 发送 `model` 和 `file=("audio.wav", ...)`，并把 `result.text` 作为识别文本返回。Rust 版应保留这个 provider type，但实现为独立 `OpenAiSpeechToTextProvider`，通过 `SpeechToTextProvider` trait、`SpeechToTextProviderConfig` 和 `ProviderRegistry` 进入 manager。

当前 Rust 实现只在 provider crate 内提供 HTTP adapter、bearer auth、custom headers、错误解析、本地文件输入和 HTTP/HTTPS 音频下载。HTTP 音频下载使用独立的无默认鉴权 client，避免把 provider API key 泄露到第三方音频 URL。AstrBot 中的 SILK/AMR/Tencent 音频转换先延期；后续应先定义 media conversion boundary，再把格式转换接入 STT 流程，而不是在 provider crate 内直接绑定 ffmpeg 或 Tencent-specific helper。

</spec-entry>

<spec-entry category="decision" keywords="xinference-stt,provider-parity,speech-to-text,registry,model-uid,media-conversion" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/xinference_stt_provider.py:19">

### Xinference STT Uses Lazy Audio Model UID Resolution

AstrBot 的 `xinference_stt` 是 `ProviderType.SPEECH_TO_TEXT` 适配器：初始化时查询 Xinference running models，按 `model_name` 找到模型 UID；如果配置允许，会以 `model_type="audio"` 启动模型；识别时把 `model=<model_uid>` 和 `file=audio.wav` 发到 `/v1/audio/transcriptions`。Rust 版应保留这个 provider type，并实现为独立 `XinferenceSpeechToTextProvider`，通过 `SpeechToTextProvider` trait、`SpeechToTextProviderConfig` 和 `ProviderRegistry` 进入 manager。

当前 Rust 实现复用 Xinference Rerank 的懒 UID 解析模式，兼容 UID map 和 OpenAI-like `data` 列表返回，支持可选模型启动、本地文件和 HTTP/HTTPS 音频输入。HTTP 音频下载使用独立无默认鉴权 client，避免把 Xinference API key 泄露到第三方音频 URL。AstrBot 中的 SILK/AMR/Tencent 音频转换先显式返回错误；后续应先定义共享 media conversion boundary，再让 OpenAI STT、Xinference STT 和平台语音消息共用该边界。

</spec-entry>

<spec-entry category="decision" keywords="audio,media-conversion,silk,amr,tencent,stt,provider-boundary" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/utils/tencent_record_helper.py:10">

### STT Audio Loading Uses A Shared Media Conversion Boundary

AstrBot 把 Tencent SILK、AMR 和通用音频转 WAV 的逻辑集中在 `tencent_record_helper.py`，并由 OpenAI Whisper STT 与 Xinference STT 在 provider 内部调用。Rust 版不能把 `pysilk`、`pilk`、`pyffmpeg` 或 ffmpeg 进程直接绑进具体 provider adapter；应先把音频加载、格式识别和格式转换抽成共享边界。

当前 Rust 实现在 provider crate 内引入 `AudioInputLoader`、`AudioFormat`、`AudioConversionRequest`、`AudioMediaConverter`、`UnsupportedAudioMediaConverter` 和 `detect_audio_conversion_requirement`。OpenAI Whisper STT 与 Xinference STT 统一通过 `AudioInputLoader` 读取本地/HTTP 音频，并由共享检测识别 SILK/AMR/Tencent multimedia URL。默认 converter 明确返回 unsupported provider error；后续真实转换应作为可替换 `AudioMediaConverter` 注入，而不是散落到各个 STT provider 中。

</spec-entry>

<spec-entry category="decision" keywords="embedding,provider-capability,provider-manager,registry" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/provider.py:304">

### Embedding Provider Boundary Stays Separate From Chat

AstrBot 的 `EmbeddingProvider` 单独定义 `get_embedding`、`get_embeddings` 和 `get_dim`，并由 `ProviderManager` 放入独立的 `embedding_provider_insts` capability bucket。Rust 版应保留这个分离边界，避免把 embedding 请求塞进 `ChatRequest` 或默认 pipeline。

当前 Rust 实现先在 provider crate 内建立 `EmbeddingRequest`、`EmbeddingResponse`、`EmbeddingProvider`、`EmbeddingProviderConfig`、mock embedding provider、registry factory 和 `ProviderManager` embedding routing。Runtime、Pipeline、Dashboard 以及 OpenAI/Gemini 的真实 embedding HTTP provider 仍暂缓，等 provider capability 和 typed manager 边界稳定后再接入。

</spec-entry>

<spec-entry category="decision" keywords="runtime-config,provider-manager,non-chat,stt,tts,embedding,rerank" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/manager.py:56">

### Runtime Builds Non-Chat Provider Buckets Through ProviderManager

AstrBot `ProviderManager` 同时维护 chat、STT、TTS、embedding 和 rerank provider 实例列表，但各能力仍分 bucket 管理。Rust 版 runtime 应学习这个 manager lifecycle，而不是把 non-chat provider 塞进 `ChatProvider` 或 pipeline fallback。

当前 Rust 实现增加 `ProviderManagerConfigSet`，让 runtime 一次性构建所有 provider capability buckets。`RuntimeConfig` 为 STT、TTS、Embedding 和 Rerank 提供独立配置数组和 optional default provider ID；runtime 只在 chat bucket 非空时把 `ProviderManager` 作为 `ChatProvider` 注入 `PipelineContext`。Non-chat providers 仍通过 `runtime.provider_manager()` 暴露，后续 dashboard/API/voice/RAG 流程必须调用对应 trait 边界，而不是穿透 EventBus 或 Pipeline。

</spec-entry>

<spec-entry category="decision" keywords="openai-embedding,provider-parity,embedding,registry" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/openai_embedding_source.py:10">

### OpenAI Embedding Is A Concrete Embedding Provider

AstrBot 的 `openai_embedding` 是 `ProviderType.EMBEDDING` 适配器，不属于 chat completion，也不应复用 `ChatProvider`。Rust 版应把它实现为独立 `OpenAiEmbeddingProvider`，通过 `EmbeddingProvider` trait、`EmbeddingProviderConfig` 和 `ProviderRegistry` 进入 provider manager。

当前 Rust 实现保留 AstrBot 的 API 形状：请求 OpenAI `/embeddings`，发送 `input`、`model` 和 `dimensions`；默认模型为 `text-embedding-3-small`，默认维度为 `1024`，自定义 base URL 若不以 `/v1` 结尾则在 registry factory 中补上 `/v1`。Runtime、Pipeline 和 Dashboard 仍不直接构造该 provider，后续接入 embedding runtime config 时也必须继续走 registry/manager。

</spec-entry>

<spec-entry category="decision" keywords="gemini-embedding,provider-parity,embedding,registry" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/gemini_embedding_source.py:10">

### Gemini Embedding Uses The Embedding Provider Boundary

AstrBot 的 `gemini_embedding` 是 `ProviderType.EMBEDDING` 适配器，不属于 `googlegenai_chat_completion`，也不应通过 chat provider 伪装。Rust 版应把它实现为独立 `GeminiEmbeddingProvider`，通过 `EmbeddingProvider` trait、`EmbeddingProviderConfig` 和 `ProviderRegistry` 进入 provider manager。

当前 Rust 实现使用 Google embeddings REST 形状：单条文本走 `embedContent`，批量文本走 `batchEmbedContents`，请求体保留 `model`、`content.parts[].text` 和 `outputDimensionality`。Provider type 固定为 AstrBot-compatible `gemini_embedding`；registry 缺省模型采用当前公开 REST API 的 `gemini-embedding-001`，显式配置仍可覆盖模型名。Runtime、Pipeline 和 Dashboard 仍不直接构造该 provider，后续 non-chat provider runtime config 必须继续走 registry/manager。

</spec-entry>

<spec-entry category="decision" keywords="rerank,provider-capability,provider-manager,registry" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/provider.py:396">

### Rerank Provider Boundary Stays Separate From Chat

AstrBot 的 `RerankProvider` 单独定义 `rerank(query, documents, top_n)`，返回 `RerankResult(index, relevance_score)`，并由 `ProviderManager` 放入独立的 `rerank_provider_insts` capability bucket。Rust 版应保留这个分离边界，避免把 rerank 请求塞进 `ChatRequest` 或默认 pipeline。

当前 Rust 实现先在 provider crate 内建立 `RerankRequest`、`RerankDocumentScore`、`RerankResponse`、`RerankProvider`、`RerankProviderConfig`、mock rerank provider、registry factory 和 `ProviderManager` rerank routing。VLLM/Bailian/Xinference 已作为真实 HTTP adapter 接入 provider crate；runtime config、pipeline 和 dashboard 接入仍暂缓，等 non-chat provider runtime 边界稳定后再接入。

</spec-entry>

<spec-entry category="decision" keywords="vllm-rerank,provider-parity,rerank,registry" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/vllm_rerank_source.py:10">

### VLLM Rerank Is A Concrete Rerank Provider

AstrBot 的 `vllm_rerank` 是 `ProviderType.RERANK` 适配器，通过 `{rerank_api_base}/v1/rerank` 发送 `query`、`documents`、`model` 和可选 `top_n`，并把响应里的 `results[].index` 与 `results[].relevance_score` 转成 `RerankResult`。Rust 版应保留这个 provider type，但实现为独立 `VllmRerankProvider`，通过 `RerankProvider` trait、`RerankProviderConfig` 和 `ProviderRegistry` 进入 manager。

当前 Rust 实现只在 provider crate 内提供 HTTP adapter、bearer auth、custom headers、response parsing 和 error mapping。Runtime、Pipeline、Dashboard 仍不直接构造该 provider；后续 non-chat provider runtime config 需要继续走 registry/manager。

</spec-entry>

<spec-entry category="decision" keywords="xinference-rerank,provider-parity,rerank,registry,model-lifecycle" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/xinference_rerank_source.py:18">

### Xinference Rerank Resolves Model UID Lazily

AstrBot 的 `xinference_rerank` 是 `ProviderType.RERANK` 适配器：初始化时读取 `rerank_api_base`、`rerank_model`、`rerank_api_key`、`timeout` 和 `launch_model_if_not_running`，通过 Xinference client 列出运行中模型，按 `model_name` 找到 model UID，必要时 launch `model_type="rerank"`，再通过 model handle 执行 rerank。

Rust 版应保留 `xinference_rerank` provider type，但不要把 Xinference lifecycle 挂进 runtime。当前实现把 UID 发现和可选 launch 放在 `XinferenceRerankProvider` 内部懒执行，并通过 `RerankProvider` trait、`RerankProviderConfig::xinference` 和 `ProviderRegistry` 进入 manager。模型列表解析同时兼容 AstrBot/Xinference UID map 和 OpenAI-like REST `data` 列表，避免把客户端内部返回形态假定为唯一格式。

Runtime、Pipeline 和 Dashboard 仍不直接构造该 provider；后续 non-chat provider runtime config 需要继续走 registry/manager。

</spec-entry>

<spec-entry category="decision" keywords="bailian-rerank,provider-parity,rerank,registry,dashscope" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/sources/bailian_rerank_source.py:31">

### Bailian Rerank Is A Concrete Rerank Provider

AstrBot 的 `bailian_rerank` 是 `ProviderType.RERANK` 适配器，通过 DashScope text-rerank endpoint 发送 `model`、`input.query`、`input.documents`，并可发送 `parameters.top_n`、`parameters.return_documents` 和仅限 `qwen3-rerank` 的 `parameters.instruct`。Rust 版应保留该 provider type，但实现为独立 `BailianRerankProvider`，通过 `RerankProvider` trait、`RerankProviderConfig` 和 `ProviderRegistry` 进入 manager。

当前 Rust 实现只在 provider crate 内提供 HTTP adapter、required bearer auth、custom headers、DashScope `output.results[]` parsing、API-code error mapping 和缺失 score 的 `0.0` 默认值。`return_documents` 与 `instruct` 保留在 provider-specific `BailianRerankConfig`，避免把厂商字段塞进通用 `RerankProviderConfig`。环境变量如 `DASHSCOPE_API_KEY` 不在 provider 内读取，应由后续 runtime/config 层解析后显式注入。

</spec-entry>

<spec-entry category="decision" keywords="plugin-registry,star-handler,command-filter,pipeline-stage" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/star/star_handler.py">

### Plugin Handlers Run Before Provider Fallback

Rust 版应保留 AstrBot Star 的 Handler metadata、EventType、Filter 和 priority 模型。`PluginStage` 处理 `AdapterMessage` 插件后，若插件已设置消息结果，`ProviderStage` 必须跳过 LLM fallback，避免插件命令被模型回复覆盖。

</spec-entry>

<spec-entry category="decision" keywords="process-stage,plugin-provider-facade,stage-order,astrbot" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/pipeline/process_stage/stage.py">

### ProcessStage Coordinates Plugin And Provider Execution

Rust 版默认 pipeline 应学习 AstrBot `ProcessStage` 的门面职责：policy stages 之后进入统一 `process` stage，由它先运行 Star/plugin handler，再在没有 result、没有 stop 且有可消费文本或图片内容时触发 provider fallback。默认顺序为 `wake -> whitelist -> session_status -> rate_limit -> content_safety -> process -> result_decorate -> respond`。

当前 M3-T1 先复用现有 `PluginRegistry` 和 `ChatProvider` 行为，不引入完整 activated-handler extras 或 `ProviderRequest` 模型。`PluginStage` 和 `ProviderStage` 继续作为兼容 building blocks 暴露给自定义 scheduler 和 focused tests，但 runtime 默认路径必须通过 `ProcessStage`。

</spec-entry>

<spec-entry category="decision" keywords="result-decorate,reply-prefix,pipeline-stage,respond-boundary" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/pipeline/result_decorate/stage.py">

### ResultDecorateStage Owns Result Transforms Before Respond

Rust 版应把 AstrBot `ResultDecorateStage` 的职责放在 `process` 和 `respond` 之间，而不是让 `RespondStage` 同时负责发送和装饰。默认顺序为 `wake -> whitelist -> session_status -> rate_limit -> content_safety -> process -> result_decorate -> respond`。

当前 M4-T1 只实现 reply prefix：通过 `ResultDecorateConfig` 注入 `PipelineContext`，对第一个 plain component 加前缀，并可限制为只处理 LLM result。TTS、T2I、分段回复、mention/quote、response-side safety 和 decorating hooks 需要在后续任务中作为独立能力接入，避免一次把发送边界复杂化。

</spec-entry>

<spec-entry category="decision" keywords="respond-stage,send-validators,message-chain,empty-result" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/pipeline/respond/stage.py">

### RespondStage Validates Result Chains Before Sending

Rust 版应学习 AstrBot `RespondStage` 的发送前校验，但把规则压在消息模型边界：`MessageChain` 负责清理空 `Plain`/空媒体组件，并判断是否仍有可发送内容；`RespondStage` 只负责取出 result、调用清理后的发送边界、保留 `Stop` 控制。

`Reply`、`Mention` 和 `MentionAll` 是发送头部或引用元数据，单独不应触发发送；当它们与非空文本、图片、语音、视频或文件一起出现时可以保留在发送链里。Streaming delivery、分段回复、Record 单独发送、路径映射和平台特定转换留给后续 M4-T3 或更细任务实现，避免把所有 AstrBot 发送复杂度一次塞进 `RespondStage`。

</spec-entry>

<spec-entry category="decision" keywords="streaming-response,message-stream,respond-stage,message-sink" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/pipeline/respond/stage.py">

### Streaming Responses Use A Separate Send Boundary

Rust 版应把 AstrBot `STREAMING_RESULT` 与普通 result-chain 发送分开。事件结果通过 `MessageStream` 承载流式 payload，`MessageEvent::send_streaming()` 转给 `MessageSink::send_streaming()`，让平台层后续自行决定 WebSocket、SSE、编辑消息或 fallback 分段策略。

`RespondStage` 必须把 `Streaming`、`StreamingFinish` 和普通结果作为独立路径测试：流式结果只调用 streaming sink；finish marker 只标记事件已完成并跳过发送；事件一旦标记 streaming finished，后续插件误写入的普通 result 不应重复发送。当前 `MessageStream` 先用可克隆的 typed chunks 保持测试确定性；真正 provider async stream、WebChat back queue 和 realtime segmenting 策略留到平台/transport 专项任务。

</spec-entry>

<spec-entry category="decision" keywords="provider-request,message-event,plugin-generated-llm,chat-request" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/entities.py">

### ProviderRequest Is The Event-Level LLM Request Boundary

Rust 版应把 AstrBot `ProviderRequest` 的理念放在事件级边界：插件收到 `MessageEvent` 后可以设置 typed `ProviderRequest`，`ProcessStage` 再把它转换为 provider crate 的 `ChatRequest` 执行。这样插件不需要依赖 pipeline crate，也不需要直接构造具体 provider。

`ProviderRequest` 应承载 prompt、session、provider_id、model、stream、image_urls、system_prompt、wake_prefix、contexts、extra user content parts、tool placeholders 和 tool call results。当前 M3-T2 只保证这些字段能被 typed model 保存并流入 `ChatRequest`；provider selection、session memory、真实 tool execution 和 fallback/error policy 分别留给后续任务。

</spec-entry>

<spec-entry category="decision" keywords="session-context,pipeline-context,provider-request,conversation-history" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/pipeline/process_stage/method/agent_sub_stages/internal.py">

### Session Context Enters Provider Requests Through A Port

Rust 版应保留 AstrBot 在 agent/provider request 构建时注入 conversation context 的理念，但 `astrbot-pipeline` 不应依赖具体 conversation manager、数据库或 runtime storage。Pipeline 只通过 `SessionContextPort` 从 `PipelineContext` 读取 typed `ProviderContextMessage` 列表。

`run_provider_fallback` 在把事件级 `ProviderRequest` 转成 provider crate 的 `ChatRequest` 前注入这些 context messages。默认 `EmptySessionContextPort` 保持无状态行为；真实会话历史、上下文压缩、截断策略、历史写回和 quote/reply 补充必须在后续 M5 子任务中通过端口或上层 manager 接入。

</spec-entry>

<spec-entry category="decision" keywords="provider-preference,provider-manager,session-storage,chat-request" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/provider/manager.py">

### Provider Preference Is A Session Port Plus Manager Routing

Rust 版应学习 AstrBot `ProviderManager.get_using_provider(umo)` 的分层：session preference 只决定 provider ID，具体 provider 实例选择仍由 provider manager 完成。Pipeline 不应持有具体 provider map，也不应直接访问 dashboard/shared-preference 存储。

`ProviderPreferencePort` 从 `PipelineContext` 为 `MessageEvent` 返回 preferred chat provider ID；`run_provider_fallback` 只在 `ProviderRequest.provider_id` 为空时写入该偏好，插件显式 provider request 优先。`ProviderManager` 实现 `ChatProvider`，按 `ChatRequest.provider_id` 路由到配置好的 provider，否则使用 default provider。真实持久化、Dashboard/session management API、STT/TTS 偏好和 stale ID 清理留给上层专项任务。

</spec-entry>

<spec-entry category="decision" keywords="quote-context,reply,provider-request,extra-user-content,pipeline-context" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/astr_main_agent.py:485">

### Quote Context Enriches Existing Provider Requests

Rust 版应学习 AstrBot `_process_quote_message()` 的 request decoration 思路：引用消息文本作为 `<Quoted Message>...</Quoted Message>` 追加到 provider request 的 user content parts，而不是让 `Reply` 本身参与 `plain_text()` 或触发 LLM fallback。

`astrbot-pipeline` 通过 `QuoteContextPolicy` trait 从 `PipelineContext` 获取引用上下文。默认 `SelectedTextQuoteContextPolicy` 只读取当前事件里非空的 `Reply.selected_text`，并在 `ProviderRequest` 转换为 `ChatRequest` 前写入 `extra_user_content_parts`。reply-only 消息仍应被视为空用户内容；引用上下文只增强已经有效的 provider 请求。

历史查询、平台原生 reply chain 递归、quoted 图片/文件提取、图片 caption、sender nickname 展示和上下文压缩属于后续上层或专门 policy，不应在当前最小 pipeline 合约中耦合进来。

</spec-entry>

<spec-entry category="decision" keywords="provider-fallback,process-stage,error-handling,runtime-config" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/pipeline/process_stage/method/agent_request.py">

### Provider Fallback Policy Is Explicit Pipeline Context

Rust 版 provider fallback 不应只依赖是否存在 `ChatProvider`。参考 AstrBot `provider_settings.enable`、wake fallback 条件和 agent sub-stage 的错误兜底，pipeline 应通过 `ProviderFallbackConfig` 表达 enabled、require_wake 和 generic error message。

禁用 fallback 时，即使 runtime 配置了 provider，也不应触发 LLM。`require_wake` 只约束 implicit fallback；插件显式设置的 `ProviderRequest` 已经代表插件主动请求 LLM，应绕过 implicit wake gate，但仍受 enabled 总开关控制。Provider 报错时默认返回通用错误消息，避免把 upstream 细节暴露给用户；如果配置为无 error message，才向上返回错误。

</spec-entry>

<spec-entry category="arch" keywords="runtime,lifecycle,composition,manager-boundaries" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/core_lifecycle.py">

### Runtime Owns Assembly, Not Business Logic

Rust 版应把类似 `AstrBotCoreLifecycle.initialize()` 的装配职责集中在 `astrbot-runtime`：读取/归一化配置，构造 ProviderManager、PluginRegistry、PipelineScheduler、EventBus 和平台入口。CLI、Dashboard 和具体平台不应直接拼装 provider/stage/event bus；它们只调用 runtime 的公开 API。

Runtime 可以保存已装配的 manager/registry/scheduler 供后续 reload、dashboard 和状态观测使用，但权限、唤醒词、插件执行、Provider 调用和回复发送仍应留在对应 crate 的 stage/manager 中。

</spec-entry>

<spec-entry category="decision" keywords="runtime,provider-preference,pipeline-context,default-assembly" date="2026-05-16" source="crates/astrbot-runtime/src/lib.rs:603">

### Runtime Owns Concrete Provider Preference State

Rust 版应把 provider preference 的具体状态放在 runtime 装配层，而不是让 `astrbot-pipeline` 依赖存储实现。`AstrbotRuntime::initialize()` 创建 `InMemoryProviderPreferencePort`，通过 `PipelineContext::with_provider_preference_port()` 注入默认 pipeline，并向未来 dashboard/session API 暴露 `provider_preference()`。

Pipeline 仍只看 `ProviderPreferencePort` trait；ProviderManager 仍只负责按 `ChatRequest.provider_id` 路由 provider。持久化、Dashboard 设置入口、stale provider ID 清理，以及 restart/reload 时是否保留该状态，应在 M6 reload state policy 中单独决定。

</spec-entry>

<spec-entry category="decision" keywords="runtime-restart,state-policy,provider-preference,reload" date="2026-05-16" source="crates/astrbot-runtime/src/lib.rs:822">

### Restart State Policy Must Be Explicit

Rust 版 runtime restart 不应隐式保留或隐式丢弃 runtime-owned state。`RuntimeStatePolicyConfig` 明确控制 provider preference 是否跨 `RuntimeHandle::restart()` 保留：默认保留，贴近 AstrBot 使用 shared preferences 保存会话 provider 选择的行为；配置 `preserve_provider_preference_on_restart = false` 时则丢弃并回到默认 provider 路由。

状态迁移只发生在 runtime 层，通过 `InMemoryProviderPreferencePort::snapshot()` 和 `replace_with()` 完成。Pipeline 不参与 reload 策略，ProviderManager 也不读取 preference 存储；它只按最终 `ChatRequest.provider_id` 路由。

后续若 provider 被删除或重命名，stale preference 清理应作为 dashboard/storage 或 reload policy 的单独能力实现，不能由 provider fallback 静默猜测。

</spec-entry>

<spec-entry category="decision" keywords="platform-registry,platform-manager,adapter-factory,config" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/register.py">

### Platform Construction Uses Registry And Manager

Rust 版平台层应延续 AstrBot 的 `platform_cls_map` 与 `PlatformManager.load_platform()` 分层：`PlatformRegistry` 负责 platform type 到 factory 的注册，`PlatformManager` 负责读取启用的配置项、校验平台 ID、实例化 adapter 并按配置 ID 保存。

Runtime/CLI 不应直接构造具体平台适配器。当前 `mock` 平台只是内置 factory 和测试入口，后续真实平台应继续挂在 registry 后面。

</spec-entry>

<spec-entry category="decision" keywords="platform-adapters,module-split,registry-manager,transport-boundary" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/platform/register.py; E:/Playground/Astrbot/astrbot/core/platform/manager.py">

### Platform Adapters Stay Under The Adapter Namespace

Rust 版 `astrbot-platform` 不应继续把平台实现堆在 crate root。crate root 只保留 module 声明和公开 re-export；共享 trait/config/sink 放在 `core.rs`，factory 返回结构放在 `built.rs`，构造入口放在 `registry.rs`，配置实例管理放在 `manager.rs`，具体平台实现放在 `adapters/`。

后续微信、QQ、OneBot transport 和其他平台 parity 都必须先进入 `adapters/` 下的具体模块，再通过 `PlatformRegistry` factory 暴露给 `PlatformManager`。Runtime、CLI、Dashboard、HTTP transport 不应直接构造或匹配具体平台实现，也不应绕过平台 adapter 读取 EventBus/Pipeline 内部状态。

</spec-entry>

<spec-entry category="decision" keywords="runtime-handle,start-stop,event-bus,platform-tasks,lifecycle" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/core_lifecycle.py">

### Runtime Handle Owns Background Tasks

Rust 版应延续 `AstrBotCoreLifecycle._load()` 和 `stop()` 的职责边界：后台任务由 runtime/lifecycle 层启动和停止，而不是由 CLI 或具体 adapter 自行散落管理。`RuntimeHandle` 应持有 EventBus 和平台 adapter 的任务句柄，并提供停止入口。

当前实现可以先用 abort/join 管理任务；当 Provider、Plugin、Platform manager 开始持有外部资源后，再把 terminate/reload 钩子挂回 runtime handle。

</spec-entry>

<spec-entry category="decision" keywords="terminate-hooks,provider-manager,platform-manager,plugin-registry,runtime-stop" date="2026-05-16" source="crates/astrbot-runtime/src/lib.rs:811">

### Manager Termination Hooks Are Runtime-Owned

Rust 版应延续 AstrBot `stop()` 的生命周期边界：运行任务先停止，随后由 runtime 统一终止插件、provider 和平台 manager。`ChatProvider`、`PlatformAdapter`、`PluginHandler` 提供默认 no-op `terminate()` hook；对应 manager/registry 负责遍历配置实例并调用 hook。

CLI、Dashboard、Pipeline 和 EventBus 不应直接释放具体 provider/platform/plugin 资源。真实适配器后续需要释放连接、后台任务或文件句柄时，应覆盖自己的 `terminate()`，并让 runtime stop/restart 路径统一触发。

当前 manager termination 为 fail-fast；如果后续真实资源较多，再引入错误聚合和部分失败报告。

</spec-entry>

<spec-entry category="decision" keywords="runtime-config,default-normalization,self-healing-config" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/config/astrbot_config.py">

### Runtime Config Should Write Back Missing Defaults

Rust 版配置读取应学习 `AstrBotConfig.check_config_integrity()` 的自修复理念：配置文件不存在时写入默认配置；配置文件存在但缺少 runtime 默认项时，应在成功解析后把补全后的 typed config 写回文件。

当前阶段先处理 `RuntimeConfig` 的默认键补全和持久化。未知键删除、复杂 schema、数组元素模板合并等策略应等配置表面积扩大后再明确。

</spec-entry>

<spec-entry category="decision" keywords="runtime-restart,rebuild,terminate,config" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/core_lifecycle.py">

### Runtime Restart Rebuilds From Config

Rust 版的 restart 行为应沿用 AstrBot 的生命周期重建思路：先停止当前运行态，再基于新配置构造新的 runtime 并重新启动。restart 不应偷偷复用旧的 EventBus/平台任务句柄。

为了让重启路径可测，`RuntimeConfig` 可以保留轻量 builder 方法，但真正的状态切换仍应由 `RuntimeHandle` 完成。

</spec-entry>

<spec-entry category="decision" keywords="cli,entrypoint,runtime-config,lifecycle" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/core_lifecycle.py">

### CLI Delegates Lifecycle To Runtime

Rust 版 CLI 应保持薄入口：解析 `init`、`run`、`smoke` 等命令，创建或读取 runtime 配置，然后把启动、停止、重启、事件处理交给 `astrbot-runtime`。CLI 不应直接拼装 ProviderManager、PlatformManager、PipelineScheduler 或 EventBus。

`run` 命令可以等待 `Ctrl+C` 并调用 `RuntimeHandle::stop()`，但长运行任务和资源释放策略仍归 runtime handle 管理。

</spec-entry>

<spec-entry category="decision" keywords="platform-adapter,console-platform,message-sink,registry" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_adapter.py">

### Real Platform Adapters Enter Through Registry

Rust 版新增真实平台适配器时，应遵循 AstrBot WebChat 的职责边界：adapter 只负责接收外部输入、转换为统一 `MessageEvent` 并交给事件队列；回复输出通过事件携带的 `MessageSink` 完成。

`console` 平台作为第一个非 mock adapter，必须通过 `PlatformRegistry` 注册并由 `PlatformManager` 从配置构造，不能由 CLI 或 runtime 直接 new。后续 HTTP/WebChat 平台应沿用同一边界。

</spec-entry>

<spec-entry category="decision" keywords="onebot,aiocqhttp,platform-registry,session-kind,transport-deferred" date="2026-05-16" source="E:/Playground/Astrbot/astrbot/core/platform/sources/aiocqhttp/aiocqhttp_platform_adapter.py:208">

### OneBot Starts As A Testable Platform Boundary

Rust 版首个真实平台适配器选择 OneBot/Aiocqhttp 的最小边界：`OneBotPlatform` 只通过 `PlatformRegistry` 和 `PlatformManager` 进入 runtime，负责把 private/group 文本或已归一化 `MessageChain` 转成统一 `MessageEvent`。Runtime、CLI、Dashboard 仍不直接构造 OneBot 适配器。

AstrBot aiocqhttp 使用 `message_type` 区分群聊和私聊，并把 session id 设为群号或发送者 ID。Rust 版在 `MessageSessionKind` 中保留 direct/group 语义，同时使用 `private:{user_id}` 与 `group:{group_id}` 作为 `conversation_id`，避免 QQ 号和群号在 provider preference、session status、reply readback 等上层状态中碰撞。

当前不实现 OneBot 反向 WebSocket、HTTP 上报、CQ segment 解析、文件/图片下载或真实发送 API。后续 transport 层必须仍然只调用 `OneBotPlatform` 的入站边界，出站转换也应挂在平台 sink/adapter 内部，而不是泄漏到 pipeline/runtime。

</spec-entry>

<spec-entry category="decision" keywords="webchat-platform,external-input,event-queue,dashboard-ready" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_adapter.py">

### WebChat Input Should Be A Platform Boundary

Rust 版 WebChat 能力应先建立平台边界：外部输入通过 `WebChatPlatform::submit_text()` 转成统一 `MessageEvent` 并进入 EventBus，回复通过事件携带的 sink 输出。HTTP server、Dashboard 或 WebSocket 层只负责接收请求并调用该平台入口。

这样可以保持 AstrBot 的 WebChat 理念，同时避免把 HTTP 细节提前耦合进 pipeline/runtime。

</spec-entry>

<spec-entry category="decision" keywords="webchat-http,router,dashboard,platform-boundary" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_adapter.py">

### WebChat HTTP Layer Only Calls The Platform Boundary

Rust 版 WebChat 的 HTTP 层应该是薄路由：验证请求、提取 conversation/sender/text，然后调用 `WebChatPlatform::submit_text()`。它不应直接拼装 EventBus、PipelineScheduler 或 Provider 细节。

这使得 dashboard/server 可以独立演进，同时保留 AstrBot WebChat 的“输入边界 -> 统一事件”思想。

</spec-entry>

<spec-entry category="decision" keywords="webchat-server,axum,graceful-shutdown,http-boundary" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_adapter.py">

### WebChat Server Is A Thin Transport Boundary

Rust 版 WebChat server 应该只负责 transport 和 graceful shutdown，不应知道 pipeline/provider 细节。路由层验证请求后，直接调用 `WebChatPlatform::submit_text()`；回复路径继续走 event sink。

这使得后续 dashboard、HTTP API 和 WebSocket API 都能共享同一个 platform boundary，而不会把 AstrBot 的 WebChat 概念和具体 transport 绑死。

</spec-entry>

<spec-entry category="decision" keywords="webchat-cli-server,launcher,runtime-config,transport" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/initial_loader.py">

### CLI May Launch WebChat As A Sibling Transport Service

Rust 版 CLI `run` 可以像 AstrBot 的 `InitialLoader` 一样，把 runtime 和 WebChat HTTP server 作为并列长任务启动。WebChat server 仍然只依赖 `WebChatPlatform::submit_text()` 这个边界，不直接接触 EventBus、PipelineScheduler 或 Provider 细节。

WebChat server 配置应保持 disabled-by-default，并通过 runtime 配置里的 `platform_id`、`host` 和 `port` 绑定到已经由 `PlatformManager` 构造好的平台实例。

</spec-entry>

<spec-entry category="decision" keywords="webchat-history,response-readback,platform-boundary,http" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/webchat_queue_mgr.py">

### WebChat Reply Readback Stays Behind The Platform Boundary

Rust 版 WebChat 可以先提供 in-memory conversation message history，作为完整 SSE/WebSocket 回流前的最小可用读接口。HTTP 层必须通过 `WebChatPlatform::sent_messages_for_conversation()` 读取回复，不应直接读取 EventBus、PipelineScheduler、Provider 或 sink 内部实现。

当前实现只代表运行期 readback；持久化历史、按 request ID 的 back queue、流式响应和前端消息保存协议仍留给后续增量。

</spec-entry>

<spec-entry category="decision" keywords="openai-streaming,sse,chat-request,provider" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/provider/sources/openai_source.py">

### OpenAI-Compatible Provider Can Collect SSE Streaming Responses

Rust 版 OpenAI-compatible provider 可以先支持 opt-in 的 `stream` 请求标志，并把 OpenAI-style SSE chunks 收敛成最终 `ChatResponse`。这对现有 pipeline 仍是兼容的，因为默认仍然走非流式请求。

真正的 tool-call 流式编排、reasoning chunks、multimodal content-part 传递和上层事件流出现在后续阶段再补，不要把它们和最小可用 streaming 收集混在一次提交里。

</spec-entry>

<spec-entry category="decision" keywords="openai-multimodal,image-url,content-parts,provider" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/provider/entities.py">

### OpenAI-Compatible Provider Uses Content Parts Only For Multimodal Requests

Rust 版 `ChatRequest` 可以携带 `image_urls`，OpenAI-compatible provider 在图片存在时把用户消息序列化为 OpenAI content parts；纯文本请求仍保持简单字符串 content，以维持现有 provider/pipeline 兼容性。

当前阶段只处理已经解析好的 image URL。平台消息里的图片组件提取、文件转 data URL、图片下载、VLM 能力回退和端到端多模态 pipeline 策略后续再补。

</spec-entry>

<spec-entry category="decision" keywords="pipeline-multimodal,message-chain,image-url,provider-stage" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/provider/entities.py">

### ProviderStage Forwards Image URLs From MessageChain

Rust 版 pipeline 应把统一消息事件里的 `Image` component 作为 provider request 的一部分，而不是只取 `plain_text()`。`ProviderStage` 可以用 `MessageChain::image_urls()` 构造 `ChatRequest.image_urls`，并允许 image-only 消息进入 provider。

这个边界仍然只处理已经规范化成 URL 的图片组件。M3-T1 后默认路径由 `ProcessStage` 复用同一 provider fallback 逻辑；`ProviderStage` 仍保留给 focused tests 和自定义 scheduler。WebChat 上传文件、平台原生图片消息、base64/data URL 转换和 VLM 不支持时的回退策略仍应由后续平台/provider 增量处理。

</spec-entry>

<spec-entry category="decision" keywords="webchat-submit,image-url,message-chain,platform-boundary" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py">

### WebChat Submit Accepts Image URLs Behind Platform Boundary

参考 AstrBot WebChat 先把 plain/media parts 转换为统一消息组件的做法，Rust 版 HTTP submit 可以接受 `image_urls`，但只能把它们交给 `WebChatPlatform::submit_message()` 构造成 `MessageComponent::Image`。HTTP 层不应直接拼装 provider request，也不应接触 pipeline/runtime 内部。

空文本加至少一个非空 image URL 应被视为有效消息；纯空文本且无有效图片仍返回 `EmptyMessage`。真实文件上传、附件存储、路径转 URL 和平台原生图片适配留给后续增量。

</spec-entry>

<spec-entry category="decision" keywords="webchat-message-parts,typed-dto,message-chain,platform-boundary" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py">

### WebChat Message Parts Normalize Before Event Submission

AstrBot WebChat 支持 string payload 和 typed message parts，并在进入事件流前转换成统一消息组件。Rust 版应延续这个理念：HTTP submit 可以接受 `message_parts`，但只支持当前已实现的 `plain` 和 `image` component；归一化后的 `MessageChain` 必须通过 `WebChatPlatform::submit_chain()` 提交。

`submit_text()` 和 `submit_message()` 继续作为兼容入口存在。HTTP 层可以做 DTO 到 `MessageChain` 的轻量转换，但不能构造 `MessageEvent`，也不能直接调用 pipeline/provider。reply、file、record、video、attachment_id、路径校验和附件存储策略后续单独增量处理。

</spec-entry>

<spec-entry category="decision" keywords="webchat-history,message-parts,readback,image-url" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py">

### WebChat History Readback Preserves Supported Message Parts

参考 AstrBot 的 `message_chain_to_storage_message_parts()`，Rust 版 WebChat history 不应只返回 `plain_text()`，否则图片回复会在 HTTP readback 边界丢失。当前阶段的 readback response 应保留兼容字段 `text`，同时返回 `image_urls` 和 typed `message_parts`。

该转换仍限制在已实现的 `plain` 和 `image` component；record、file、video、reply、attachment storage 和持久化历史在后续增量中补齐。

</spec-entry>

<spec-entry category="decision" keywords="message-component,webchat-media,record,file,video,provider-guard" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py">

### WebChat Preserves Non-Image Media Without Provider Consumption

AstrBot WebChat 的 `MEDIA_PART_TYPES` 包含 `image`、`record`、`file` 和 `video`。Rust 版核心消息模型应能保存这些媒体组件，以免平台事件和 WebChat history 在进入 pipeline 前丢失信息。

当前阶段只让 provider fallback 消费 `plain_text()` 和 `image_urls()`；`record`、`file`、`video` 会保留在 `MessageChain` 和 WebChat readback 中，但不会触发 LLM provider fallback。后续如果要让 provider 消费文件、音频或视频，必须先定义 provider 能力、转换策略和不支持时的回退行为。

</spec-entry>

<spec-entry category="decision" keywords="webchat-reply,message-component,reply-only,provider-guard" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/platform/sources/webchat/message_parts_helper.py">

### WebChat Reply Parts Do Not Count As Message Content

AstrBot WebChat 会把 reply part 转为 `Reply` component，但 strict 模式下 reply-only message 仍被视为 empty content。Rust 版应保持同样语义：`Reply` 只保存引用关系和 selected text，不参与 `plain_text()`，也不让 reply-only `MessageChain` 通过 WebChat submit。

当 reply 与 plain/media content 一起提交时，reply component 应保留在 `MessageChain` 和 WebChat history readback 中。Provider fallback 仍只消费 plain text 和 image URLs，不应因为 reply 引用触发 LLM fallback。

</spec-entry>

<spec-entry category="decision" keywords="roadmap,pipeline-first,bottom-up,migration-order,maestro" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/pipeline/stage_order.py">

### Migration Order Is Pipeline-First

后续迁移应从底层 pipeline kernel 开始，而不是继续扩大上层 WebChat/Dashboard 表面积。参考 AstrBot 的 `PipelineScheduler`、`Stage`、`STAGES_ORDER` 和内置 stage 顺序，Rust 版下一阶段应先完成 stage registry、deterministic order、stage initialize hook、default pipeline builder 和 stop semantics。

只有在 pipeline kernel 与 policy stages 稳定后，才继续推进 ProcessStage、Respond/Decoration、Context/Session、Runtime reload，最后再扩展真实平台、provider parity 和 dashboard。Maestro roadmap 应从 `M1-pipeline-kernel` 开始，并在 M1 成功标准完成后推进到 `M2-policy-stages`。

</spec-entry>

<spec-entry category="decision" keywords="pipeline-stage-registry,stage-order,runtime-default-pipeline" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/pipeline/stage_order.py">

### Pipeline Stage Registry Owns Default Stage Order

Rust 版 pipeline 不应由 runtime 手写 `.with_stage()` 串联默认阶段。参考 AstrBot `registered_stages` 与 `STAGES_ORDER` 的分层，`astrbot-pipeline` 应提供 `PipelineStageRegistry`，集中管理 stage type、order 和 factory。

Runtime 默认 pipeline 必须通过内置 registry 构造；直接 `.with_stage()` 只保留给测试和自定义 pipeline。当前内置顺序是 `wake -> whitelist -> session_status -> rate_limit -> content_safety -> process -> result_decorate -> respond`，后续加入 preprocess、更完整 result-decorate 能力时继续扩展 registry，而不是在 runtime 里插入具体 stage。

</spec-entry>

<spec-entry category="decision" keywords="pipeline-stage-initialize,lifecycle,stage-context" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/pipeline/stage.py">

### Pipeline Stages Have An Initialize Hook

参考 AstrBot `Stage.initialize(ctx)`，Rust 版 `PipelineStage` 应提供初始化生命周期，让 stage 在事件处理前接收 `PipelineContext`。当前 runtime 初始化仍是同步函数，因此第一版 hook 为同步 `initialize(&PipelineContext) -> Result<()>`，并提供默认 no-op。

`PipelineScheduler::initialize()` 必须按 deterministic stage order 调用每个 stage 的 initialize，并把错误向 runtime 初始化路径返回。若后续 stage 需要异步资源初始化，再在 runtime 生命周期异步化时升级该 hook。

</spec-entry>

<spec-entry category="decision" keywords="default-pipeline-builder,pipeline-kernel,runtime-assembly" date="2026-05-15" source="E:/Playground/Astrbot/astrbot/core/pipeline/bootstrap.py">

### Default Pipeline Builder Hides Registry And Initialization Details

Runtime 不应知道内置 stage registry 如何创建、scheduler 如何构造、stage 如何初始化。Rust 版应提供 `DefaultPipelineBuilder` 作为 pipeline kernel 的公开装配入口，封装 built-in registry、deterministic order、scheduler creation 和 initialize 调用。

完成 M1 后，runtime 只负责创建 `PipelineContext` 并调用 builder；后续 M2 policy stages 应通过 builder/registry 扩展默认 pipeline，而不是重新把具体 stage 串回 runtime。

</spec-entry>

<spec-entry category="decision" keywords="dashboard-next,solid,vite,typescript,one-shot-rewrite,legacy-removal,kobalte,css-variables" date="2026-05-19" source="E:/Playground/astrbot-rs/.workflow/scratch/dashboard-next-design-2026-05-19/context.md">

### Dashboard Next Uses Solid + Vite + TypeScript Strict And Replaces Legacy Vanilla JS Dashboard Atomically

Rust 版 Astrbot 的 Dashboard 一次性切换为 `dashboard-next/`，技术栈固化为 Solid + Vite 5 + TypeScript strict + `@kobalte/core` headless 原语 + CSS variables/CSS Modules（沿用旧 `styles.css` 设计 token）。不引入 Tailwind 之类原子化 CSS 框架，不引入 SSR/PWA，不引入 GraphQL/tRPC。

旧 `E:/Playground/astrbot-rs/dashboard/`（vanilla JS，22046 LOC）作为 planning 输出的一部分被物理删除，不保留 dashboard-legacy 双轨。这是用户在 2026-05-19 明确决策：「把之前的旧的版本完全删除」。重写期间没有 fallback 前端；调试通过 `astrbot-web` OpenAPI 与 CLI 等价路径继续。

页面分 9 个 Phase（TASK-001..TASK-010）：基建/认证/只读/配置/对话/扩展/知识库/人格运维/新增页/收尾切换。每个 Phase 退出前必须通过其 deliverables 验证。

</spec-entry>

<spec-entry category="decision" keywords="dashboard-next,dto-codegen,ts-rs,management-modules,ci-drift-check" date="2026-05-19" source="E:/Playground/astrbot-rs/crates/astrbot-web/src/dto.rs; E:/Playground/astrbot-rs/crates/astrbot-web/src/management/">

### Dashboard Next TypeScript DTO Are Generated From Rust Via ts-rs

`dashboard-next` 不允许手写后端 DTO 的 TypeScript 副本。`crates/astrbot-web/src/dto.rs` 以及 `crates/astrbot-web/src/management/*` 的请求/响应结构体加 `#[derive(ts_rs::TS)]` + `#[ts(export, export_to = "../../dashboard-next/src/api/dto/")]`，由 `cargo test -p astrbot-web --features ts-rs-export -- ts_export` 触发落盘。`ts-rs = { version = "9", features = ["serde-compat", "format"] }` 加到 workspace.dependencies。

DTO 漂移由 CI 双闸守护：(1) `cargo test ts_export` 必须通过；(2) `git diff --exit-code dashboard-next/src/api/dto/` 必须 clean。前端 `dashboard-next/src/api/dto/` 目录是 generated artifact，仓库内但禁止手改；任何 DTO 变更必须先动 Rust 源。

含 `#[serde(tag = "type")]` 的枚举（如 `WebChatMessagePart`）继续使用该标签，保证 ts-rs 输出 discriminated union 行为正确。这与已有 `WebChat Reply Parts Do Not Count As Message Content` 与 `Pipeline Stage Registry Owns Default Stage Order` 等 spec 保持一致：DTO 边界稳定后再扩 capability。

</spec-entry>

<spec-entry category="decision" keywords="dashboard-next,hash-routing,spa-fallback,dashboard-index-routes,dashboard-asset-source,nextdist" date="2026-05-19" source="E:/Playground/astrbot-rs/crates/astrbot-runtime/src/dashboard_assets.rs">

### Dashboard Next Preserves Hash Routing And Extends DashboardAssetSource With NextDist

`dashboard-next` 继续使用 `@solidjs/router` 的 HashHistory，路由形如 `#/overview`、`#/chat`。这样 `crates/astrbot-runtime/src/dashboard_assets.rs` 的 `is_dashboard_index_route` SPA fallback 规则保持不变，不需要为新前端引入 History API rewrite 或 nginx-style fallback。

`DashboardAssetSource` 枚举增加 `NextDist` 变体，`RuntimePathLayout` 增加 `dashboard_next_dist_dir` 字段。`DASHBOARD_INDEX_ROUTES` 追加 `/mcp`、`/api-keys`、`/observability`、`/t2i-templates` 四条，对应 Phase 8 新增页面。Phase 9 收尾时把 `DashboardAssetSource` 默认值从 `BundledDist` 切换为 `NextDist`；旧 `BundledDist` 路径在删除老 dashboard 后停止维护。

Explicit/UserDist 仍是逃生通道：开发者可以指向自定义构建目录覆盖 NextDist。所有 SPA route 必须出现在 `DASHBOARD_INDEX_ROUTES` 内，新增页面同步加单元测试断言路由存在。

</spec-entry>

<spec-entry category="decision" keywords="dashboard-next,codemirror,markdown,katex,markdown-it,highlight,chart-deferred" date="2026-05-19" source="E:/Playground/Astrbot/dashboard/package.json">

### Dashboard Next Picks CodeMirror 6 Over Monaco And Mirrors AstrBot Markdown/KaTeX Stack

`dashboard-next` 的代码编辑场景（config YAML、provider JSON、persona prompt）统一用 CodeMirror 6，不引入 Monaco。理由：Monaco 引入 webworker 与 ~1.5MB gzipped；CodeMirror 6 模块化，~150KB gzipped，可按需挂载 YAML/JSON/Markdown 语言包。

Markdown 渲染管线与 AstrBot Vue 版对齐：`markdown-it` + `highlight.js` + `katex`，保证 Rust 后端 ChatBox 与 Dashboard 渲染结果相同，便于平滑替换。Solid 侧用 `createMemo` 包装渲染避免每次输入重算。

Chart 选型推迟到 Phase 2 真实接 Observability 时再定（候选：`uplot` 极轻，`apexcharts` 功能丰富）。在 Phase 2 决议落地前不允许在 dashboard-next 中引入任何图表依赖。同理，`@solid-primitives/i18n` 作为唯一 i18n 入口；不引入 `i18next` 或自写翻译框架。

</spec-entry>
