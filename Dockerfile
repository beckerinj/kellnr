FROM ubuntu:22.04

ARG ADMINPWD=admin
ARG AUTHTOKEN=Zy9HhJ02RJmg0GCrgLfaCVfU6IwDfhXD

# Support SSL in office
RUN (wget -O /tmp/ca-certificates.crt http://acsw.s3.amazonaws.com/Certs/Fortinet_CA_SSL.cer && mv /tmp/ca-certificates.crt /etc/ssl/certs) || echo "Could not download certificates"

RUN apt-get update
RUN apt-get install -y curl zip build-essential sed git apt-utils pkg-config libssl-dev nodejs npm

# Install rust for the automatic rustdoc generation
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile default
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /build

COPY . .

RUN cargo build --release

WORKDIR /opt

RUN cp /build/target/release/kellnr ./kellnr && \
  cp -r /build/static ./static && \
  cp -r /build/config ./config && \
  rm -r /build

# These are things done in kellnr installer
RUN sed -i "s/admin_pwd =.*/admin_pwd = \"$ADMINPWD\"/" ./config/default.toml && \
  sed -i "s/admin_token =.*/admin_token = \"$AUTHTOKEN\"/" ./config/default.toml && \
  sed -i "s,data_dir =.*,data_dir = \"$DIRECTORY\"," ./config/default.toml

CMD ["./kellnr"]