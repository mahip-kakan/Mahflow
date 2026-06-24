import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Mic, AudioLines, Loader2, X } from "lucide-react";
import "./RecordingOverlay.css";
import { commands } from "@/bindings";
import i18n, { syncLanguageFromSettings } from "@/i18n";
import { getLanguageDirection } from "@/lib/utils/rtl";

type OverlayState = "recording" | "transcribing" | "processing";

/// Split the running transcript into "history" (older sentences, dimmed) and
/// "current" (the sentence being dictated right now, emphasized). We anchor on
/// the *last* sentence so the active line stays bold even after its closing
/// punctuation appears, until a new sentence begins. If there's only one
/// sentence so far, the whole thing is "current" — so the first line is bold,
/// not faded.
const SENTENCE_BOUNDARY = /[.!?।]\s+/g;

function splitActiveSentence(text: string): {
  history: string;
  current: string;
} {
  const trimmedLen = text.trimEnd().length;
  let splitAt = 0;
  let match: RegExpExecArray | null;
  SENTENCE_BOUNDARY.lastIndex = 0;
  while ((match = SENTENCE_BOUNDARY.exec(text)) !== null) {
    const end = match.index + match[0].length;
    // Ignore a boundary that sits at the very end (trailing punctuation),
    // otherwise the active sentence would momentarily become empty.
    if (end < trimmedLen) {
      splitAt = end;
    }
  }
  return { history: text.slice(0, splitAt), current: text.slice(splitAt) };
}

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  const [levels, setLevels] = useState<number[]>(Array(16).fill(0));
  const [partialText, setPartialText] = useState<string>("");
  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));
  const tickerRef = useRef<HTMLDivElement>(null);
  const direction = getLanguageDirection(i18n.language);

  // Keep the newest line visible: whenever the live text grows, glide the
  // vertical scroll to the bottom (the words just spoken). Older lines scroll
  // up and fade under the top mask — the "Spotify lyrics" feel.
  useEffect(() => {
    const el = tickerRef.current;
    if (!el) return;
    const id = requestAnimationFrame(() => {
      el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
    });
    return () => cancelAnimationFrame(id);
  }, [partialText]);

  useEffect(() => {
    const setupEventListeners = async () => {
      // Listen for show-overlay event from Rust
      const unlistenShow = await listen("show-overlay", async (event) => {
        // Sync language from settings each time overlay is shown
        await syncLanguageFromSettings();
        const overlayState = event.payload as OverlayState;
        setState(overlayState);
        setIsVisible(true);
        // Clear any stale live-preview text whenever the state changes
        // (e.g. recording -> transcribing) and on a fresh recording.
        setPartialText("");
      });

      // Listen for hide-overlay event from Rust
      const unlistenHide = await listen("hide-overlay", () => {
        setIsVisible(false);
        setPartialText("");
      });

      // Listen for live transcription preview updates
      const unlistenPartial = await listen<string>(
        "partial-transcription",
        (event) => {
          setPartialText(event.payload as string);
        },
      );

      // Listen for mic-level updates
      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const newLevels = event.payload as number[];

        // Apply smoothing to reduce jitter
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3; // Smooth transition
        });

        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed.slice(0, 9));
      });

      // Cleanup function
      return () => {
        unlistenShow();
        unlistenHide();
        unlistenLevel();
        unlistenPartial();
      };
    };

    setupEventListeners();
  }, []);

  const getIcon = () => {
    switch (state) {
      case "recording":
        return <Mic size={20} strokeWidth={2} className="overlay-icon" />;
      case "transcribing":
        return (
          <AudioLines
            size={20}
            strokeWidth={2}
            className="overlay-icon overlay-icon-accent"
          />
        );
      case "processing":
        return (
          <Loader2
            size={20}
            strokeWidth={2}
            className="overlay-icon overlay-icon-accent overlay-spin"
          />
        );
    }
  };

  // Live "caption card" mode: while recording with live-preview text, the pill
  // expands into a multi-line paragraph card anchored to the bottom.
  const isLiveCard = state === "recording" && !!partialText;

  return (
    <div className="overlay-root" dir={direction}>
      <div
        className={`recording-overlay ${isVisible ? "fade-in" : ""} ${
          isLiveCard ? "is-live-card" : ""
        }`}
        data-state={state}
      >
        <div className="overlay-left">{getIcon()}</div>

        <div className="overlay-middle">
          {isLiveCard &&
            (() => {
              const { history, current } = splitActiveSentence(partialText);
              return (
                <div
                  className="live-paragraph"
                  ref={tickerRef}
                  title={partialText}
                >
                  <div className="live-paragraph-inner">
                    {history && <span className="lp-history">{history}</span>}
                    <span className="lp-current">{current}</span>
                  </div>
                </div>
              );
            })()}
          {state === "recording" && !partialText && (
            <div className="bars-container">
              {levels.map((v, i) => (
                <div
                  key={i}
                  className="bar"
                  style={{
                    height: `${Math.min(20, 4 + Math.pow(v, 0.7) * 16)}px`, // Cap at 20px max height
                    transition: "height 60ms ease-out, opacity 120ms ease-out",
                    opacity: Math.max(0.2, v * 1.7), // Minimum opacity for visibility
                  }}
                />
              ))}
            </div>
          )}
          {state === "transcribing" && (
            <div className="status-indicator">
              <span className="status-text">{t("overlay.transcribing")}</span>
              <span className="status-dots" aria-hidden="true">
                <i />
                <i />
                <i />
              </span>
            </div>
          )}
          {state === "processing" && (
            <div className="status-indicator">
              <span className="status-text">{t("overlay.processing")}</span>
            </div>
          )}
        </div>

        <div className="overlay-right">
          {state === "recording" && (
            <button
              type="button"
              className="cancel-button"
              aria-label="Cancel"
              onClick={() => {
                commands.cancelOperation();
              }}
            >
              <X size={16} strokeWidth={2.25} className="overlay-icon" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

export default RecordingOverlay;
