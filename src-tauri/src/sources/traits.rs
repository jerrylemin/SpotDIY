use crate::domain::{ProviderKind, SourceCapabilities};
use crate::search::types::{
    ProviderRuntimeStatus, ProviderSearchRequest, ProviderSearchSection, SearchCancellation,
    SearchEntityKind,
};
use std::future::Future;
use std::pin::Pin;

pub trait SourceAdapter: Send + Sync {
    fn kind(&self) -> ProviderKind;
    fn capabilities(&self) -> SourceCapabilities;
    fn supported_entities(&self) -> &'static [SearchEntityKind];
    fn runtime_status(&self) -> ProviderRuntimeStatus;
    fn search(
        &self,
        request: ProviderSearchRequest,
        cancellation: SearchCancellation,
    ) -> Pin<Box<dyn Future<Output = ProviderSearchSection> + Send + '_>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_adapter_is_object_safe() {
        fn assert_object_safe(_: &dyn SourceAdapter) {}
        let _ = assert_object_safe;
    }
}
