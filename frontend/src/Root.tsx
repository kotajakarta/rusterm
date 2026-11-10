import { useCallback, useEffect, useState } from "react";

import { api, setSession, setUnauthorizedHandler } from "./api";
import { App } from "./App";
import { ForceChangePinScreen } from "./components/ForceChangePinScreen";
import { LockScreen } from "./components/LockScreen";
import { SetupPin } from "./components/SetupPin";

type Gate = "checking" | "setup" | "locked" | "force-change-pin" | "unlocked";

/** Shows the setup/PIN screen until the backend accepts a session, then the app. */
export function Root() {
  const [gate, setGate] = useState<Gate>("checking");

  useEffect(() => {
    setUnauthorizedHandler(() => setGate("locked"));
    api
      .authStatus()
      .then((status) => {
        if (!status.initialized) {
          setGate("setup");
          return;
        }
        if (status.session_valid) {
          if (status.must_change_pin) {
            setGate("force-change-pin");
          } else {
            setGate("unlocked");
          }
        } else {
          setSession("");
          setGate("locked");
        }
      })
      .catch(() => setGate("locked"));
    return () => setUnauthorizedHandler(null);
  }, []);

  const lock = useCallback(async () => {
    try {
      await api.lock();
    } finally {
      setGate("locked");
    }
  }, []);

  if (gate === "checking") return null;
  if (gate === "setup") return <SetupPin onCreated={() => setGate("unlocked")} />;
  if (gate === "force-change-pin") return <ForceChangePinScreen onUpdated={() => setGate("unlocked")} />;
  if (gate === "locked") {
    return (
      <LockScreen
        onUnlocked={(mustChangePin) => {
          if (mustChangePin) {
            setGate("force-change-pin");
          } else {
            setGate("unlocked");
          }
        }}
      />
    );
  }
  return <App onLock={lock} />;
}
