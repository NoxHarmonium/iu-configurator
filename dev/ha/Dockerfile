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
ARG IU_CARD_VERSION=2026.1.0

# curl is available in the HA base image (Alpine); install unzip from apk.
# The GitHub source archive extracts to irrigation_unlimited-<version>/ so we
# move the nested custom_components/irrigation_unlimited dir to the right place.
RUN apk add --no-cache unzip && \
    curl -fsSL \
        "https://github.com/rgc99/irrigation_unlimited/archive/refs/tags/${IU_VERSION}.zip" \
        -o /tmp/iu.zip && \
    unzip -q /tmp/iu.zip -d /tmp/iu_extract && \
    mkdir -p /config/custom_components && \
    mv /tmp/iu_extract/irrigation_unlimited-${IU_VERSION}/custom_components/irrigation_unlimited \
       /config/custom_components/irrigation_unlimited && \
    rm -rf /tmp/iu.zip /tmp/iu_extract

# Install the Irrigation Unlimited companion card as a Lovelace custom resource.
# HA serves files under /config/www/ at the /local/ path.
RUN mkdir -p /config/www && \
    curl -fsSL \
        "https://github.com/rgc99/irrigation-unlimited-card/releases/download/${IU_CARD_VERSION}/irrigation-unlimited-card.js" \
        -o /config/www/irrigation-unlimited-card.js

# Seed the initial Home Assistant configuration into the image layer.
# This is picked up by Docker when the ha-config named volume is first created.
COPY config/ /config/
