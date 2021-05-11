# Note: We don't use Alpine and its packaged Rust/Cargo because they're too often out of date,
# preventing them from being used to build Substrate/Polkadot.

FROM phusion/baseimage:0.11 as builder
LABEL maintainer="chris.dcosta@totemaccounting.com"
LABEL description="This is the build stage for Totem Lego. Here we create the binary."

ENV DEBIAN_FRONTEND=noninteractive

ARG PROFILE=release
WORKDIR /totem-lego

COPY . /totem-lego

RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
	export PATH="$PATH:$HOME/.cargo/bin" && \
	# rustup toolchain install nightly && \
	rustup toolchain install nightly-2021-03-01 && \
	# rustup target add wasm32-unknown-unknown --toolchain nightly && \
	rustup target add wasm32-unknown-unknown --toolchain nightly-2021-03-01 && \
	# rustup default stable && \
	rustup default nightly-2021-03-01 && \
	cargo build "--$PROFILE"

# ===== SECOND STAGE ======

FROM phusion/baseimage:0.11
LABEL maintainer="chris.dcosta@totemaccounting.com"
LABEL description="This is the 2nd stage: a very small image where we copy the Totem Lego binary."
ARG PROFILE=release

RUN mv /usr/share/ca* /tmp && \
	rm -rf /usr/share/*  && \
	mv /tmp/ca-certificates /usr/share/ && \
	useradd -m -u 1000 -U -s /bin/sh -d /totem-lego lego && \
	mkdir -p /totem-lego/.local/share/totem-lego && \
	chown -R lego:lego /totem-lego/.local && \
	ln -s /totem-lego/.local/share/totem-lego /data

COPY --from=builder /totem-lego/target/$PROFILE/substrate /usr/local/bin
# COPY --from=builder /substrate/target/$PROFILE/subkey /usr/local/bin
# COPY --from=builder /substrate/target/$PROFILE/node-rpc-client /usr/local/bin
# COPY --from=builder /substrate/target/$PROFILE/node-template /usr/local/bin
# COPY --from=builder /substrate/target/$PROFILE/chain-spec-builder /usr/local/bin

# checks
RUN ldd /usr/local/bin/substrate && \
	/usr/local/bin/substrate --version

# Shrinking
RUN rm -rf /usr/lib/python* && \
	rm -rf /usr/bin /usr/sbin /usr/share/man

USER lego
EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]

CMD ["/usr/local/bin/substrate"]