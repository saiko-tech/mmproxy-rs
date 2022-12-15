# mmproxy-rs

Rust implementation of MMProxy

## Features

- [x] TCP - Accepts [PROXY Protocol](https://www.haproxy.org/download/1.8/doc/proxy-protocol.txt) enabled requests from [Nginx](https://docs.nginx.com/nginx/admin-guide/load-balancer/using-proxy-protocol/#proxy-protocol-for-a-tcp-connection-to-an-upstream), [HAProxy](https://www.haproxy.org/download/1.8/doc/proxy-protocol.txt)
- [x] UDP - Accepts PROXY Protocol enabled requests from [udppp](https://github.com/b23r0/udppp)

## Usage

```
$ mmproxy -h
Usage: mmproxy [-h] [options]

Options:
  -h, --help              Prints the help string.
  -4, --ipv4 <addr>       Address to which IPv4 traffic will be forwarded to.
                          (default: "127.0.0.1:443")
  -6, --ipv6 <addr>       Address to which IPv6 traffic will be forwarded to.
                          (default: "[::1]:443")

  -a, --allowed-subnets <path>
                          Path to a file that contains allowed subnets of the
                          proxy servers.

  -c, --close-after <n>   Number of seconds after which UDP socket will be
                          cleaned up. (default: 60)

  -l, --listen-addr <string>
                          Address the proxy listens on. (default:
                          "0.0.0.0:8443")

  --listeners <n>         Number of listener sockets that will be opened for the
                          listen address. (Linux 3.9+) (default: 1)
  -p, --protocol <p>      Protocol that will be proxied: tcp, udp. (default:
                          tcp)
  -m, --mark <n>          The mark that will be set on outbound packets.
                          (default: 0)
```

### Example

:warning: needs root permissions

```terminal
address=X.X.X.X # get this via "ip addr" command - don't use 0.0.0.0!
bind_port=8080
upstream_port=8081
sudo ip rule add from 127.0.0.1/8 iif lo table 123
sudo ip route add local 0.0.0.0/0 dev lo table 123
sudo mmproxy  -l $address:$bind_port -4 127.0.0.1:$upstream_port -p udp
```

## Benchmarking

See https://gist.github.com/brkp/f3026a37a2ce869493483d5dbfb2ce02

An initial benchmark gave the following results:

```
# go-mmproxy: [send: 20512950][recv: 352875]
# mmproxy-rs: [send: 19705328][recv: 593526]
```

This seems to suggest this rust implementation has about 70% more throughput than `go-mmproxy` for `upstream->downstream` and has comparable performance for `downstream->upstream`.

## Acknowledgements and References

- https://blog.cloudflare.com/mmproxy-creative-way-of-preserving-client-ips-in-spectrum/
- https://github.com/cloudflare/mmproxy
- https://github.com/b23r0/udppp
- https://github.com/path-network/go-mmproxy
