//! Who sent a request, for the limit on jobs per client
//! (`--max-jobs-per-client`).
//!
//! Behind a proxy every connection comes from the proxy, so the client's
//! address is read from the header the proxy sets (`--client-ip-header`,
//! e.g. `CF-Connecting-IP` behind Cloudflare). Anyone can send that header
//! too, so it is only trustworthy when the server cannot be reached except
//! through the proxy (`docs/DEPLOY.md`).

use std::convert::Infallible;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::HeaderName;
use axum::http::request::Parts;

/// The header holding the client's address, as a request extension; `None`
/// uses the connection's address.
#[derive(Debug, Clone)]
pub struct ClientIpHeader(pub Option<HeaderName>);

/// The client a request comes from: its IPv4 address, or the /64 network
/// of its IPv6 address, as one home or phone is handed a whole /64 and
/// could otherwise take a new address for every job. `None` when unknown,
/// as for a router served without connection info (tests): such requests
/// are not limited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client(pub Option<IpAddr>);

impl<S: Send + Sync> FromRequestParts<S> for Client {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Infallible> {
        let from_header = parts
            .extensions
            .get::<ClientIpHeader>()
            .and_then(|header| header.0.as_ref())
            .and_then(|name| parts.headers.get(name))
            .and_then(|value| value.to_str().ok())
            // A list (`X-Forwarded-For`) ends with the address the proxy
            // nearest to the server saw; the ones before it are as the
            // client sent them.
            .and_then(|value| value.rsplit(',').next())
            .and_then(|address| address.trim().parse::<IpAddr>().ok());
        let from_connection = || {
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.0.ip())
        };
        Ok(Client(from_header.or_else(from_connection).map(network)))
    }
}

/// The address that stands for the client: an IPv4 address as is, an IPv6
/// one cut to its /64 network.
fn network(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V4(_) => address,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => {
                let network = u128::from(v6) & !(u128::from(u64::MAX));
                IpAddr::V6(Ipv6Addr::from(network))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ipv6_client_is_its_64_network() {
        let one: IpAddr = "2001:db8:1:2:aaaa::1".parse().unwrap();
        let other: IpAddr = "2001:db8:1:2:bbbb::9".parse().unwrap();
        assert_eq!(network(one), network(other));
        assert_eq!(network(one), "2001:db8:1:2::".parse::<IpAddr>().unwrap());
        let v4: IpAddr = "203.0.113.7".parse().unwrap();
        assert_eq!(network(v4), v4);
        let mapped: IpAddr = "::ffff:203.0.113.7".parse().unwrap();
        assert_eq!(network(mapped), v4);
    }
}
