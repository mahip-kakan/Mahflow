import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ArrowDownRight,
  ArrowUpRight,
  BookText,
  Flame,
  Mic,
  TrendingUp,
} from "lucide-react";
import { commands, type UsageInsights } from "@/bindings";

/** Map a day's word count to a 0–4 intensity bucket relative to the busiest
 * day in the window, so the heatmap scales to each user's own activity. */
function intensityLevel(words: number, max: number): number {
  if (words <= 0 || max <= 0) return 0;
  const ratio = words / max;
  if (ratio > 0.75) return 4;
  if (ratio > 0.5) return 3;
  if (ratio > 0.25) return 2;
  return 1;
}

const LEVEL_CLASSES = [
  "bg-mid-gray/10",
  "bg-background-ui/30",
  "bg-background-ui/55",
  "bg-background-ui/80",
  "bg-background-ui",
];

interface StatCardProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  sub?: React.ReactNode;
}

const StatCard: React.FC<StatCardProps> = ({ icon, label, value, sub }) => (
  <div className="glass-card p-4 flex flex-col gap-2">
    <div className="flex items-center gap-2 text-mid-gray">
      <span className="opacity-80">{icon}</span>
      <span className="text-xs font-medium uppercase tracking-wide">
        {label}
      </span>
    </div>
    <div className="text-3xl font-semibold leading-none">{value}</div>
    {sub && <div className="text-xs text-mid-gray">{sub}</div>}
  </div>
);

export const InsightsSettings: React.FC = () => {
  const { t } = useTranslation();
  const [insights, setInsights] = useState<UsageInsights | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const result = await commands.getUsageInsights();
      if (cancelled) return;
      if (result.status === "ok") {
        setInsights(result.data);
        setError(null);
      } else {
        setError(result.error);
      }
      setLoading(false);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Build GitHub-style week columns from the trailing daily activity. We pad
  // the start so the first column begins on a Sunday.
  const weeks = useMemo(() => {
    if (!insights) return [] as (UsageInsights["daily_activity"][number] | null)[][];
    const days = insights.daily_activity;
    if (days.length === 0) return [];
    const firstWeekday = new Date(`${days[0].date}T00:00:00`).getDay(); // 0 = Sun
    const padded: (UsageInsights["daily_activity"][number] | null)[] = [
      ...Array(firstWeekday).fill(null),
      ...days,
    ];
    const cols: (UsageInsights["daily_activity"][number] | null)[][] = [];
    for (let i = 0; i < padded.length; i += 7) {
      cols.push(padded.slice(i, i + 7));
    }
    return cols;
  }, [insights]);

  const maxWords = useMemo(() => {
    if (!insights) return 0;
    return insights.daily_activity.reduce((m, d) => Math.max(m, d.words), 0);
  }, [insights]);

  if (loading) {
    return (
      <div className="max-w-3xl w-full mx-auto py-16 text-center text-mid-gray">
        {t("settings.insights.loading")}
      </div>
    );
  }

  if (error) {
    return (
      <div className="max-w-3xl w-full mx-auto py-16 text-center text-mid-gray">
        {t("settings.insights.error")}
      </div>
    );
  }

  if (!insights || insights.total_recordings === 0) {
    return (
      <div className="max-w-3xl w-full mx-auto py-16 text-center space-y-2">
        <Mic className="mx-auto text-mid-gray" size={32} />
        <h2 className="text-lg font-semibold">
          {t("settings.insights.empty.title")}
        </h2>
        <p className="text-sm text-mid-gray">
          {t("settings.insights.empty.description")}
        </p>
      </div>
    );
  }

  const nf = new Intl.NumberFormat();
  const pct = insights.month_change_pct;
  const pctRounded = pct === null ? null : Math.round(pct);

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <div className="px-1">
        <h1 className="text-2xl font-semibold">{t("settings.insights.title")}</h1>
        <p className="text-sm text-mid-gray">
          {t("settings.insights.subtitle")}
        </p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <StatCard
          icon={<TrendingUp size={16} />}
          label={t("settings.insights.totalWords")}
          value={nf.format(insights.total_words)}
          sub={
            pctRounded === null ? (
              t("settings.insights.thisMonthWords", {
                count: insights.words_this_month,
              })
            ) : (
              <span
                className={`inline-flex items-center gap-1 ${
                  pctRounded >= 0 ? "text-emerald-500" : "text-rose-500"
                }`}
              >
                {pctRounded >= 0 ? (
                  <ArrowUpRight size={12} />
                ) : (
                  <ArrowDownRight size={12} />
                )}
                {t("settings.insights.changeThisMonth", {
                  pct: Math.abs(pctRounded),
                })}
              </span>
            )
          }
        />
        <StatCard
          icon={<Mic size={16} />}
          label={t("settings.insights.totalRecordings")}
          value={nf.format(insights.total_recordings)}
          sub={t("settings.insights.avgWords", {
            count: insights.avg_words_per_recording,
          })}
        />
        <StatCard
          icon={<Flame size={16} />}
          label={t("settings.insights.currentStreak")}
          value={t("settings.insights.days", {
            count: insights.current_streak_days,
          })}
          sub={t("settings.insights.longestStreak", {
            count: insights.longest_streak_days,
          })}
        />
      </div>

      {/* Activity heatmap */}
      <div className="glass-card p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold">
            {t("settings.insights.activity")}
          </h2>
          <span className="text-xs text-mid-gray">
            {t("settings.insights.activeDays", {
              count: insights.active_days,
            })}
          </span>
        </div>
        <div className="flex gap-[3px] overflow-x-auto pb-1">
          {weeks.map((week, wi) => (
            <div key={wi} className="flex flex-col gap-[3px]">
              {Array.from({ length: 7 }).map((_, di) => {
                const day = week[di] ?? null;
                if (!day) {
                  return <div key={di} className="w-3 h-3 rounded-sm" />;
                }
                const level = intensityLevel(day.words, maxWords);
                return (
                  <div
                    key={di}
                    title={`${day.date} · ${nf.format(day.words)} ${t(
                      "settings.insights.wordsLabel"
                    )}`}
                    className={`w-3 h-3 rounded-sm ${LEVEL_CLASSES[level]}`}
                  />
                );
              })}
            </div>
          ))}
        </div>
        <div className="flex items-center justify-end gap-1 text-[10px] text-mid-gray">
          <span>{t("settings.insights.less")}</span>
          {LEVEL_CLASSES.map((cls, i) => (
            <span key={i} className={`w-3 h-3 rounded-sm ${cls}`} />
          ))}
          <span>{t("settings.insights.more")}</span>
        </div>
      </div>

      {/* Dictionary / corrections summary */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <StatCard
          icon={<BookText size={16} />}
          label={t("settings.insights.dictionaryWords")}
          value={nf.format(insights.dictionary_words)}
          sub={t("settings.insights.dictionaryHint")}
        />
        <StatCard
          icon={<BookText size={16} />}
          label={t("settings.insights.learnedCorrections")}
          value={nf.format(insights.learned_corrections)}
          sub={t("settings.insights.learnedHint")}
        />
      </div>

      <p className="text-xs text-mid-gray px-1">
        {t("settings.insights.privacyNote")}
      </p>
    </div>
  );
};
