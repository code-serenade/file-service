.DEFAULT_GOAL := all

.PHONY: all help build-docker-release package-docker-offline show-docker-bundle upload-docker-bundle

all: build-docker-release upload-docker-bundle

help:
	@echo "Available commands:"
	@echo "  make build-docker-release   - Build the amd64 file-service image and offline deploy tarball"
	@echo "  make package-docker-offline - Package the existing amd64 Docker image"
	@echo "  make show-docker-bundle     - Show the offline deploy tarball path"
	@echo "  make upload-docker-bundle   - Upload the offline tarball to the default server"

build-docker-release:
	@./deploy/build-release.sh
	@echo "Deploy tar: $(abspath dist/file-service-linux-x86_64-docker.tar.gz)"

package-docker-offline:
	@./deploy/package-offline.sh
	@echo "Deploy tar: $(abspath dist/file-service-linux-x86_64-docker.tar.gz)"

show-docker-bundle:
	@if [ -f "dist/file-service-linux-x86_64-docker.tar.gz" ]; then \
		echo "Deploy tar: $(abspath dist/file-service-linux-x86_64-docker.tar.gz)"; \
	else \
		echo "Deploy tar has not been built yet"; \
		exit 1; \
	fi

upload-docker-bundle:
	@./deploy/upload.sh
