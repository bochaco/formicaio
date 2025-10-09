# Build and push the Formica image
docker build -t bochaco/formica:latest --platform linux/amd64,linux/arm64 -f formica.Dockerfile --push --no-cache .

# Build and push the Formicaio image
docker build -t bochaco/formicaio:latest --build-arg BUILD_ARGS='--features native' --platform linux/amd64,linux/arm64 --push .