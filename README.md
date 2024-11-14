<h1 align="center">
Traceloop Hub
</h1>
<p align="center">
  <p align="center">Open-source, high-performance LLM gateway written in Rust. Connect to any LLM provider with a single API. Observability Included.</p>
</p>
<h4 align="center">
    <a href="https://traceloop.com/docs/hub/getting-started"><strong>Get started »</strong></a>
    <br />
    <br />
  <a href="https://traceloop.com/slack">Slack</a> |
  <a href="https://traceloop.com/docs/hub">Docs</a>
</h4>

<h4 align="center">
  <a href="https://github.com/traceloop/hub/releases">
    <img src="https://img.shields.io/github/release/traceloop/hub">
  </a>
   <a href="https://github.com/traceloop/hub/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-Apache 2.0-blue.svg" alt="Traceloop Hub is released under the Apache-2.0 License">
  </a>
  <a href="https://github.com/traceloop/hub/actions/workflows/ci.yml">
  <img src="https://github.com/traceloop/hub/actions/workflows/ci.yml/badge.svg">
  </a>
  <a href="https://github.com/traceloop/hub/issues">
    <img src="https://img.shields.io/github/commit-activity/m/traceloop/hub" alt="git commit activity" />
  </a>
  <a href="https://www.ycombinator.com/companies/traceloop"><img src="https://img.shields.io/website?color=%23f26522&down_message=Y%20Combinator&label=Backed&logo=ycombinator&style=flat-square&up_message=Y%20Combinator&url=https%3A%2F%2Fwww.ycombinator.com"></a>
  <a href="https://github.com/traceloop/hub/blob/main/CONTRIBUTING.md">
    <img src="https://img.shields.io/badge/PRs-Welcome-brightgreen" alt="PRs welcome!" />
  </a>
  <a href="https://traceloop.com/slack">
    <img src="https://img.shields.io/badge/chat-on%20Slack-blueviolet" alt="Slack community channel" />
  </a>
  <a href="https://twitter.com/traceloopdev">
    <img src="https://img.shields.io/badge/follow-%40traceloopdev-1DA1F2?logo=twitter&style=social" alt="Traceloop Twitter" />
  </a>
</h4>

Hub is a next generation smart proxy for LLM applications. It centralizes control and tracing of all LLM calls and traces.
It's built in Rust so it's fast and efficient. It's completely open-source and free to use.

Built and maintained by Traceloop under the Apache 2.0 license.

## 🚀 Getting Started

You can run the hub locally by running `cargo run` in the root directory, or using the docker image:

```
docker run --rm -v $(pwd)/config.yaml:/etc/config.yaml:ro -t hub
```

## 🌱 Contributing

Whether big or small, we love contributions ❤️ Check out our guide to see how to [get started](https://traceloop.com/docs/hub/contributing/overview).

Not sure where to get started? You can:

- [Book a free pairing session with one of our teammates](mailto:nir@traceloop.com?subject=Pairing%20session&body=I'd%20like%20to%20do%20a%20pairing%20session!)!
- Join our <a href="https://traceloop.com/slack">Slack</a>, and ask us any questions there.

## 💚 Community & Support

- [Slack](https://traceloop.com/slack) (For live discussion with the community and the Traceloop team)
- [GitHub Discussions](https://github.com/traceloop/hub/discussions) (For help with building and deeper conversations about features)
- [GitHub Issues](https://github.com/traceloop/hub/issues) (For any bugs and errors you encounter using OpenLLMetry)
- [Twitter](https://twitter.com/traceloopdev) (Get news fast)
