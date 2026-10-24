//! Affine custody of one exact client's fixed-storage request.
use super::*;

/// An admitted request exclusively borrows its client until completion/cancel.
/// There is no detached, copyable identity to reset, wrap or target another client.
///
/// A second request cannot reuse a still-borrowed client:
/// ```compile_fail,E0499
/// use conduitos::http::NativeHttpClient;
/// let mut client = NativeHttpClient::prepare();
/// let request = client.begin(&[], true).unwrap();
/// let another = client.begin(&[], true);
/// let _ = request.request_bytes();
/// ```
/// Consuming cancellation cannot leave a reusable request:
/// ```compile_fail,E0382
/// use conduitos::http::NativeHttpClient;
/// let mut client = NativeHttpClient::prepare();
/// let request = client.begin(&[], true).unwrap();
/// request.cancel().unwrap();
/// let _ = request.request_bytes();
/// ```
pub struct HttpRequestGuard<'client> {
    client: &'client mut NativeHttpClient,
    transaction: u64,
    completed: bool,
}
impl<'client> HttpRequestGuard<'client> {
    pub(super) fn new(client: &'client mut NativeHttpClient, transaction: u64) -> Self {
        Self {
            client,
            transaction,
            completed: false,
        }
    }
    pub fn request_bytes(&self) -> &[u8] {
        &self.client.request[..self.client.request_len]
    }
    /// Exactly one synchronous provider attempt. Failure may have an unknown
    /// external network outcome; retirement is not network rollback or retry.
    pub fn exchange<B: HttpNetworkBase>(
        mut self,
        base: &mut B,
        output: &mut FixedHttpOutput,
    ) -> Result<(), HttpClientFailure> {
        let client = &mut self.client;
        let response_len = base
            .exchange(&client.request[..client.request_len], &mut client.response)
            .map_err(map_network)?;
        if response_len > client.response.len() {
            return Err(HttpClientFailure::ResponseBodyOverflow);
        }
        let len = encode_info_response(
            self.transaction,
            &client.response[..response_len],
            &client.response_type[..client.response_type_len],
            &mut output.bytes,
        )?;
        output.len = len;
        self.completed = true;
        Ok(())
    }
    pub fn cancel(self) -> Result<(), HttpClientFailure> {
        Ok(())
    }
}
impl Drop for HttpRequestGuard<'_> {
    fn drop(&mut self) {
        // Only this guard can clear its pending latch. Forgetting it leaves the
        // client permanently under Pressure, even when the Rust borrow ends.
        self.client.pending = false;
        self.client.request_len = 0;
        self.client.request.fill(0);
        self.client.response.fill(0);
        self.client.signs = self
            .client
            .signs
            .saturating_add(if self.completed { 2 } else { 1 });
    }
}
