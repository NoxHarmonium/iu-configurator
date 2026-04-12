# ── Home Assistant with Irrigation Unlimited pre-installed ───────────────────
#
# Irrigation Unlimited is a HACS custom component. We install it directly from
# the GitHub release zip so HACS itself is not required.
#
# Docker automatically copies image-layer files at /config into the named
# volume on first use, seeding both the custom component and configuration.
# Subsequent container restarts reuse the named volume (HA state is preserved).
#
FROM ghcr.io/home-assistant/home-assistant:stable

ARG IU_VERSION=2025.12.0

# curl is available in the HA base image (Alpine); install unzip from apk.
RUN apk add --no-cache unzip && \
    curl -fsSL \
        "https://github.com/rgc99/irrigation_unlimited/archive/refs/tags/${IU_VERSION}.zip" \
        -o /tmp/iu.zip && \
    mkdir -p /config/custom_components && \
    unzip -q /tmp/iu.zip -d /config && \
    rm /tmp/iu.zip

# Seed the initial Home Assistant configuration into the image layer.
# This is picked up by Docker when the ha-config named volume is first created.
COPY config/ /config/
