//! Body HTTP dispatch; handlers retain their existing validation and status laws.
use super::{PatchbayHtmlServer, ServerError};
use std::net::TcpStream;

impl PatchbayHtmlServer {
    pub(super) fn deliver_body_route(
        &mut self,
        request_line: &str,
        stream: &mut TcpStream,
        body: &[u8],
    ) -> Option<Result<(), ServerError>> {
        if let Some(path) = request_line
            .strip_prefix("GET /")
            .and_then(|line| line.strip_suffix(" HTTP/1.1"))
        {
            if let Some(bytes) = super::body_execution::assets::resource(path) {
                return Some(super::write_response(
                    stream,
                    "200 OK",
                    "text/javascript; charset=utf-8",
                    bytes,
                ));
            }
        }
        Some(match request_line {
            "POST /api/body-execution HTTP/1.1" => self.deliver_body_execution(stream, body),
            "POST /api/body-workload HTTP/1.1" => self.deliver_body_workload(stream, body),
            "POST /api/body-membership-evidence HTTP/1.1" => {
                self.deliver_body_membership_evidence(stream, body)
            }
            "POST /api/body-host-offer-evidence HTTP/1.1" => {
                self.deliver_body_host_offer_evidence(stream, body)
            }
            "POST /api/body-host-planning-offer HTTP/1.1" => {
                self.deliver_body_host_planning_offer(stream, body)
            }
            "GET /api/body-planning-requirements HTTP/1.1" => {
                self.deliver_body_planning_requirements(stream)
            }
            "GET /api/body-evidence HTTP/1.1" => self.deliver_body_evidence(stream),
            "GET /api/body-execution-proposal HTTP/1.1" => {
                self.deliver_body_execution_proposal(stream)
            }
            _ => return None,
        })
    }
}
