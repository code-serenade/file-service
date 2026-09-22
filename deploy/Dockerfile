FROM scratch

WORKDIR /app

COPY deploy/artifacts/file-service /usr/local/bin/file-service
COPY deploy/artifacts/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

EXPOSE 19878

CMD ["/usr/local/bin/file-service"]
