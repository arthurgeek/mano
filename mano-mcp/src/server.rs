use std::path::PathBuf;

use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};

use crate::tools::{get_translation_prompt, run_mano_code};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RunParams {
    #[schemars(description = "The mano source code to execute")]
    pub code: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TranslateParams {
    #[schemars(description = "The code or pseudocode to translate to mano")]
    pub code: String,
}

#[derive(Debug, Clone)]
pub struct ManoMcp {
    mano_bin: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl ManoMcp {
    pub fn new(mano_bin: PathBuf) -> Self {
        Self {
            mano_bin,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Execute mano code and return output")]
    pub async fn run_mano(&self, Parameters(params): Parameters<RunParams>) -> String {
        run_mano_code(&self.mano_bin, &params.code).await
    }

    #[tool(
        description = "Get mano keyword reference for translating code. Use run_mano to verify!"
    )]
    pub fn translate_to_mano(&self, Parameters(params): Parameters<TranslateParams>) -> String {
        get_translation_prompt(&params.code)
    }
}

#[tool_handler]
impl ServerHandler for ManoMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("mano interpreter - execute code written in mano slang".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
