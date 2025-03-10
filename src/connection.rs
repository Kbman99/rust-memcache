use std::io::BufReader;
use std::net::TcpStream;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::time::Duration;
#[cfg(unix)]
use url::Host;
use url::Url;

use error::MemcacheError;
use protocol::{AsciiProtocol, BinaryProtocol, Protocol};
use stream::Stream;
use stream::UdpStream;

/// a connection to the memcached server
pub struct Connection {
    pub protocol: Protocol,
    pub url: String,
}

impl Connection {
    pub(crate) fn connect(url: &Url) -> Result<Self, MemcacheError> {
        let parts: Vec<&str> = url.scheme().split("+").collect();
        if parts.len() != 1 && parts.len() != 2 || parts[0] != "memcache" {
            return Err(MemcacheError::ClientError(
                "memcache URL's scheme should start with 'memcache'".into(),
            ));
        }
        if parts.len() == 2 && !(parts[1] != "tcp" || parts[1] != "udp" || parts[1] != "unix") {
            return Err(MemcacheError::ClientError(
                "memcache URL's scheme should be 'memcache+tcp' or 'memcache+udp' or 'memcache+unix'".into(),
            ));
        }

        let is_ascii = url.query_pairs().any(|(ref k, ref v)| k == "protocol" && v == "ascii");

        let mut is_udp = url.query_pairs().any(|(ref k, ref v)| k == "udp" && v == "true");

        if parts.len() == 2 && parts[1] == "udp" {
            // scheme specify have high priority.
            is_udp = true;
        }
        if is_udp {
            let udp_stream = Stream::Udp(UdpStream::new(url.clone())?);
            if is_ascii {
                return Ok(Connection {
                    url: url.to_string(),
                    protocol: Protocol::Ascii(AsciiProtocol {
                        reader: BufReader::new(udp_stream)
                    }),
                });
            } else {
                return Ok(Connection {
                    url: url.to_string(),
                    protocol: Protocol::Binary(BinaryProtocol {
                        stream: udp_stream }),
                });
            }
        }

        #[cfg(unix)]
        {
            if url.host() == Some(Host::Domain("")) && url.port() == None {
                let unix_stream = Stream::Unix(UnixStream::connect(url.path())?);
                if is_ascii {
                    return Ok(Connection {
                        url: url.to_string(),
                        protocol: Protocol::Ascii(AsciiProtocol {
                            reader: BufReader::new(unix_stream),
                        }),
                    });
                } else {
                    return Ok(Connection {
                        url: url.to_string(),
                        protocol: Protocol::Binary(BinaryProtocol {
                            stream: unix_stream,
                        }),
                    });
                }
            }
        }

        let tcp_stream = TcpStream::connect(url.clone())?;

        let disable_tcp_nodelay = url
            .query_pairs()
            .any(|(ref k, ref v)| k == "tcp_nodelay" && v == "false");
        if !disable_tcp_nodelay {
            tcp_stream.set_nodelay(true)?;
        }

        let timeout = url
            .query_pairs()
            .find(|&(ref k, ref _v)| k == "timeout")
            .and_then(|(ref _k, ref v)| v.parse::<u64>().ok())
            .map(Duration::from_secs);
        if timeout.is_some() {
            tcp_stream.set_read_timeout(timeout)?;
            tcp_stream.set_write_timeout(timeout)?;
        }

        if is_ascii {
            return Ok(Connection {
                url: url.to_string(),
                protocol: Protocol::Ascii(AsciiProtocol {
                    reader: BufReader::new(Stream::Tcp(tcp_stream)),
                }),
            });
        }
        return Ok(Connection {
            url: url.to_string(),
            protocol: Protocol::Binary(BinaryProtocol {
                stream: Stream::Tcp(tcp_stream),
            }),
        });
    }
}
