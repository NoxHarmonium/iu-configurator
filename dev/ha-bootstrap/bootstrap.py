#!/usr/bin/env python3
"""
Bootstrap Home Assistant onboarding and create a long-lived API token.

Writes HA_TOKEN and HA_URL to /config/ha.env in KEY=VALUE format.
Exits 0 whether a token was created or skipped (idempotent).
"""

import json
import os
import sys
import time

import requests
import websocket

HA_BASE = "http://homeassistant:8123"
HA_WS = "ws://homeassistant:8123/api/websocket"
CLIENT_ID = "http://localhost:8123/"
TOKEN_FILE = "/config/ha.env"
IRRIGATION_CONFIG_FILE = "/config/irrigation_unlimited.yaml"


def log(msg):
    print(f"[bootstrap] {msg}", flush=True)


def wait_for_ha(timeout=300, interval=5):
    """Poll GET /api/onboarding until HA responds with the steps list."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            r = requests.get(f"{HA_BASE}/api/onboarding", timeout=5)
            if r.status_code == 200:
                data = r.json()
                # HA returns a flat list in recent versions
                if isinstance(data, list):
                    return data
                if isinstance(data, dict) and "result" in data:
                    return data["result"]
        except Exception:
            pass
        log("Waiting for Home Assistant to start…")
        time.sleep(interval)
    raise RuntimeError("Timed out waiting for Home Assistant to become ready")


def step_done(steps, step_name):
    return any(s.get("step") == step_name and s.get("done") for s in steps)


def complete_onboarding_step(step, access_token):
    """Complete a named onboarding step. Non-fatal on error.

    The 'integration' step requires redirect_uri for the companion-app OAuth flow.
    All other steps only need client_id.
    """
    body = {"client_id": CLIENT_ID}

    if step == "integration":
        body["redirect_uri"] = CLIENT_ID

    try:
        r = requests.post(
            f"{HA_BASE}/api/onboarding/{step}",
            json=body,
            headers={"Authorization": f"Bearer {access_token}"},
            timeout=10,
        )
        if r.status_code == 404:
            log(f"Onboarding step '{step}': not present in this HA version, skipping")
        else:
            log(f"Onboarding step '{step}': HTTP {r.status_code}")
    except Exception as e:
        log(f"Onboarding step '{step}' error (non-fatal): {e}")


def create_long_lived_token(access_token):
    """Use the HA WebSocket API to create a long-lived access token and
    register the Irrigation Unlimited card as a Lovelace resource."""
    result = {}

    def on_message(ws, message):
        data = json.loads(message)
        msg_type = data.get("type")

        if msg_type == "auth_required":
            ws.send(json.dumps({"type": "auth", "access_token": access_token}))

        elif msg_type == "auth_ok":
            ws.send(json.dumps({
                "id": 1,
                "type": "auth/long_lived_access_token",
                "client_name": "iu-configurator-dev",
                "lifespan": 3650,
            }))

        elif msg_type == "result" and data.get("id") == 1:
            if data.get("success"):
                result["token"] = data["result"]
                # Register the companion card as a Lovelace module resource.
                ws.send(json.dumps({
                    "id": 2,
                    "type": "lovelace/resources/create",
                    "res_type": "module",
                    "url": "/local/irrigation-unlimited-card.js",
                }))
            else:
                result["error"] = data.get("error", {}).get("message", "unknown")
                ws.close()

        elif msg_type == "result" and data.get("id") == 2:
            if data.get("success"):
                log("Lovelace resource registered: /local/irrigation-unlimited-card.js")
            else:
                log(f"Lovelace resource registration failed (non-fatal): {data.get('error')}")
            ws.close()

        elif msg_type == "auth_invalid":
            result["error"] = "auth_invalid"
            ws.close()

    def on_error(ws, error):
        result["error"] = str(error)

    ws = websocket.WebSocketApp(HA_WS, on_message=on_message, on_error=on_error)
    ws.run_forever()

    if "token" not in result:
        raise RuntimeError(
            f"Failed to create long-lived token: {result.get('error', 'unknown')}"
        )

    return result["token"]


def seed_irrigation_config():
    """Write a minimal irrigation_unlimited.yaml if one does not already exist.

    HA fails to start if the file referenced by `!include iu/irrigation_unlimited.yaml`
    is missing.  This seed is the smallest valid config IU accepts; iu-configurator
    overwrites it with real data on the first save.
    """
    if os.path.exists(IRRIGATION_CONFIG_FILE):
        return
    os.makedirs(os.path.dirname(IRRIGATION_CONFIG_FILE), exist_ok=True)
    with open(IRRIGATION_CONFIG_FILE, "w") as f:
        f.write("controllers: []\n")
    log(f"Seeded empty {IRRIGATION_CONFIG_FILE}")


def main():
    seed_irrigation_config()

    if os.path.exists(TOKEN_FILE):
        log(f"{TOKEN_FILE} already exists — skipping bootstrap")
        sys.exit(0)

    log("Waiting for Home Assistant…")
    steps = wait_for_ha()
    log(f"Onboarding steps from HA: {[s.get('step') for s in steps]}")

    if step_done(steps, "user"):
        log(
            "WARNING: HA 'user' onboarding step is already complete. "
            "Set HA_TOKEN manually in docker-compose.yml."
        )
        sys.exit(0)

    log("Completing onboarding 'user' step…")
    r = requests.post(
        f"{HA_BASE}/api/onboarding/users",
        json={
            "client_id": CLIENT_ID,
            "name": "Admin",
            "username": "admin",
            "password": "admin",
            "language": "en",
        },
        timeout=15,
    )
    r.raise_for_status()
    auth_code = r.json()["auth_code"]
    log("Admin user created")

    log("Exchanging auth code for short-lived access token…")
    r = requests.post(
        f"{HA_BASE}/auth/token",
        data={
            "grant_type": "authorization_code",
            "code": auth_code,
            "client_id": CLIENT_ID,
        },
        timeout=10,
    )
    r.raise_for_status()
    access_token = r.json()["access_token"]
    log("Short-lived token obtained")

    # Complete remaining onboarding steps so the browser opens to the dashboard
    for step in ("core_config", "analytics", "integration"):
        complete_onboarding_step(step, access_token)

    log("Creating long-lived token via WebSocket…")
    long_lived_token = create_long_lived_token(access_token)
    log("Long-lived token created")

    with open(TOKEN_FILE, "w") as f:
        f.write(f"HA_TOKEN={long_lived_token}\n")
        f.write(f"HA_URL={HA_BASE}\n")

    log(f"Credentials written to {TOKEN_FILE}")

    discover_weather_entity(long_lived_token)


def discover_weather_entity(access_token):
    """Find the met.no weather entity that HA auto-configures and write it to ha.env.

    Met.no is a built-in HA integration that sets itself up automatically from
    the home coordinates in configuration.yaml — no config flow needed.
    We just poll until the entity appears (it can take a few seconds after boot)
    then record it so the iu-configurator app can use it.

    Idempotent: skips if HA_WEATHER_ENTITY is already present in the token file.
    """
    try:
        existing = open(TOKEN_FILE).read()
    except OSError:
        existing = ""

    if "HA_WEATHER_ENTITY=" in existing:
        log("HA_WEATHER_ENTITY already set — skipping weather entity discovery")
        return

    headers = {
        "Authorization": f"Bearer {access_token}",
        "Content-Type": "application/json",
    }

    log("Waiting for met.no weather entity…")
    deadline = time.time() + 60
    entity_id = None

    while time.time() < deadline:
        try:
            r = requests.get(f"{HA_BASE}/api/states", headers=headers, timeout=10)
            if r.ok:
                weather_entities = [
                    s["entity_id"]
                    for s in r.json()
                    if s.get("entity_id", "").startswith("weather.")
                ]
                if weather_entities:
                    # Prefer a non-hourly entity if both exist.
                    entity_id = next(
                        (e for e in weather_entities if "hourly" not in e),
                        weather_entities[0],
                    )
                    break
        except Exception:
            pass
        time.sleep(3)

    if not entity_id:
        log("No weather.* entity found — HA_WEATHER_ENTITY not written (non-fatal)")
        return

    log(f"Found weather entity: {entity_id}")
    with open(TOKEN_FILE, "a") as f:
        f.write(f"HA_WEATHER_ENTITY={entity_id}\n")
    log(f"HA_WEATHER_ENTITY={entity_id} written to {TOKEN_FILE}")


if __name__ == "__main__":
    main()
