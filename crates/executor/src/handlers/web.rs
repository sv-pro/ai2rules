//! Web fetch handler (E7.3). Fetches a URL via the transport and returns the
//! body as an `External` output. The executor tags every result `Tainted`, so
//! fetched web content is untrusted by construction and can never go on to drive
//! egress / credentials / memory (invariant 7, floored at build).

use harness_types::ExecutionSpec;

use crate::handler::{ExecError, ExecOutput, Handler};
use crate::handlers::{str_field, structured};
use crate::transport::WebFetcher;

pub struct WebHandler {
    fetcher: Box<dyn WebFetcher>,
}

impl WebHandler {
    pub fn new(fetcher: Box<dyn WebFetcher>) -> Self {
        Self { fetcher }
    }
}

impl Handler for WebHandler {
    fn execute(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let url = str_field(structured(spec)?, "url")?;
        let content = self.fetcher.fetch(&url).map_err(ExecError::Io)?;
        Ok(ExecOutput::External {
            source: format!("web:{url}"),
            content,
        })
    }

    fn simulate(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let url = str_field(structured(spec)?, "url")?;
        Ok(ExecOutput::Simulated(format!("would fetch {url}")))
    }
}
