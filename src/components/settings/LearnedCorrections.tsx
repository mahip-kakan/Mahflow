import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands, type LearnedCorrection } from "@/bindings";
import { Button } from "../ui/Button";
import { SettingContainer } from "../ui/SettingContainer";

interface LearnedCorrectionsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

/**
 * Read/remove list for the corrections Mahflow has learned from the user's
 * edits in the History tab. New corrections are added there (with
 * confirmation); this view only lets the user review and forget them.
 */
export const LearnedCorrections: React.FC<LearnedCorrectionsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const [corrections, setCorrections] = useState<LearnedCorrection[]>([]);
    const [busy, setBusy] = useState(false);

    const load = useCallback(async () => {
      try {
        const result = await commands.getLearnedCorrections();
        setCorrections(result);
      } catch (error) {
        console.error("Failed to load learned corrections:", error);
      }
    }, []);

    useEffect(() => {
      load();
    }, [load]);

    const handleRemove = async (correction: LearnedCorrection) => {
      // Optimistic removal.
      setCorrections((prev) =>
        prev.filter(
          (c) => !(c.from === correction.from && c.to === correction.to),
        ),
      );
      try {
        setBusy(true);
        const result = await commands.removeLearnedCorrection(
          correction.from,
          correction.to,
        );
        if (result.status !== "ok") {
          toast.error(t("settings.advanced.learnedCorrections.removeError"));
          load();
        }
      } catch (error) {
        console.error("Failed to remove learned correction:", error);
        toast.error(t("settings.advanced.learnedCorrections.removeError"));
        load();
      } finally {
        setBusy(false);
      }
    };

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.learnedCorrections.title")}
          description={t("settings.advanced.learnedCorrections.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <span className="text-sm text-text/50">
            {t("settings.advanced.learnedCorrections.count", {
              count: corrections.length,
            })}
          </span>
        </SettingContainer>
        {corrections.length > 0 && (
          <div
            className={`px-4 p-2 ${
              grouped ? "" : "rounded-lg border border-mid-gray/20"
            } flex flex-wrap gap-1`}
          >
            {corrections.map((correction) => (
              <Button
                key={`${correction.from}->${correction.to}`}
                onClick={() => handleRemove(correction)}
                disabled={busy}
                variant="secondary"
                size="sm"
                className="inline-flex items-center gap-1 cursor-pointer"
                aria-label={t("settings.advanced.learnedCorrections.remove", {
                  from: correction.from,
                  to: correction.to,
                })}
                title={t("settings.advanced.learnedCorrections.remove", {
                  from: correction.from,
                  to: correction.to,
                })}
              >
                <span className="line-through text-text/50">
                  {correction.from}
                </span>
                <span className="text-text/40">→</span>
                <span>{correction.to}</span>
                <svg
                  className="w-3 h-3"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </Button>
            ))}
          </div>
        )}
      </>
    );
  },
);
