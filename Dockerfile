FROM rust:1.81

RUN apt-get update
RUN apt-get install sudo qemu-system-x86 jq -y

RUN mkdir -p /opt/shad/rust
COPY .gitlab-ci.yml /opt/shad/.grader-ci.yml

WORKDIR /opt/shad/rust
COPY . .

RUN make install
# RUN cd sem && cargo build && cargo test
# RUN cd tools && cargo build
# RUN make test

# RUN cargo run --manifest-path tools/compose/Cargo.toml -- --in-path . --out-path . \
# 	--spare tools --spare Cargo.lock --spare .compose.yml \
# 	--no-process --add-tool tools/check --add-tool tools/compose

# RUN cargo build --manifest-path tools/check/Cargo.toml

# RUN cargo run --manifest-path tools/compose/Cargo.toml -- --in-path . --out-path . \
# 	--spare tools --spare Cargo.lock --add-tool tools/check --add-tool tools/compose
