# Dockerfile for running Formicaio app on Windows

# We first just install tailwindcss with nodejs
FROM mcr.microsoft.com/powershell as tailwindcss-builder

SHELL ["powershell", "-Command", "$ErrorActionPreference = 'Stop';$ProgressPreference='silentlyContinue';"]

RUN Invoke-WebRequest -OutFile nvm-noinstall.zip -UseBasicParsing -Uri "https://github.com/coreybutler/nvm-windows/releases/latest/download/nvm-noinstall.zip"
RUN Expand-Archive -DestinationPath C:\ nvm-noinstall.zip
#RUN Rename-Item C:\node-v20.17.0-x64 C:\nodejs
#RUN SETX PATH C:\nodejs
#RUN msiexec.exe /q /i nodejs.msi
RUN C:\nvm.exe install 20.17.0
RUN C:\nvm.exe use 20.17.0

WORKDIR /app
COPY package.json package-lock.json ./

# Install tailwindcss modules
RUN npm install -D tailwindcss
RUN npx tailwindcss init

# Now let's use a build env with Rust for the app
FROM mcr.microsoft.com/devcontainers/rust:dev-1 as builder

SHELL ["powershell", "-Command", "$ErrorActionPreference = 'Stop';$ProgressPreference='silentlyContinue';"]

# Install cargo-binstall, which makes it easier to install other
# cargo extensions like cargo-leptos
# Install cargo-binstall for Windows amd64/arm64
#RUN Invoke-WebRequest -OutFile cargo-binstall.zip -UseBasicParsing -Uri "https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-pc-windows-msvc.full.zip"
#RUN Expand-Archive -DestinationPath C:\ cargo-binstall.zip
RUN Set-ExecutionPolicy Unrestricted -Scope Process; iex (iwr "https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.ps1").Content

#RUN C:\cargo-binstall.exe rustup -y
#RUN winget install Rustlang.Rustup

# Install cargo-leptos
RUN cargo-binstall cargo-leptos -y

# Add the WASM target
RUN rustup target add wasm32-unknown-unknown
RUN rustup component add rustfmt
RUN rustup component add clippy

WORKDIR /app

# Copy tailwindcss modules, and nodejs binary, to the /app directory
# since they are required for building the app
COPY --from=tailwindcss-builder /app/node_modules /app/node_modules
COPY --from=tailwindcss-builder /usr/local/bin/node /usr/local/bin/node

# Now we can copy the source files to build them
COPY . .

# make sure we exit early if clippy is not happy
#RUN cargo clippy -- -D warnings

# Build the app
RUN cargo leptos build --release -vv

# Finally use a slim Debian image to build the final runtime image 
# which contains only the built app and required resource files.
FROM mcr.microsoft.com/windows/nanoserver:1809 AS runtime
WORKDIR /app

# Copy the server binary to the /app directory
COPY --from=builder /app/target/release/formicaio.exe /app/

# Copy Sqlite migrations files
COPY --from=builder /app/migrations /app/migrations

# /target/site contains our JS/WASM/CSS, etc.
COPY --from=builder /app/target/site /app/site

# Copy Cargo.toml if it’s needed at runtime
COPY --from=builder /app/Cargo.toml /app/

# Set any required env variables and
ENV RUST_LOG="info"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
ENV LEPTOS_SITE_ROOT="site"

EXPOSE 8080

# Run the server
CMD ["/app/formicaio.exe"]
