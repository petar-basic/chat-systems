ARG POSTGRES_IMAGE=postgres:16-alpine

FROM ${POSTGRES_IMAGE} AS build
ARG PG_BIGM_VERSION=1.2-20250903
ARG PG_BIGM_SHA256=4d4fb48161e3f9e99207851299379c83b3e5d1d3c1a07c35f6a3153d577b3142
RUN apk add --no-cache build-base icu-dev
WORKDIR /src
RUN wget -q -O pg_bigm.tar.gz "https://github.com/pgbigm/pg_bigm/archive/refs/tags/v${PG_BIGM_VERSION}.tar.gz" \
 && echo "${PG_BIGM_SHA256}  pg_bigm.tar.gz" | sha256sum -c - \
 && tar xzf pg_bigm.tar.gz --strip-components=1 \
 && make USE_PGXS=1 with_llvm=no \
 && make USE_PGXS=1 with_llvm=no install DESTDIR=/out

FROM ${POSTGRES_IMAGE}
COPY --from=build /out/usr/local/ /usr/local/
CMD ["postgres", "-c", "shared_preload_libraries=pg_bigm"]
