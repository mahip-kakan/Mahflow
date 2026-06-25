import React from "react";
import { useTranslation } from "react-i18next";
import { Cog, FlaskConical, History, Info, Sparkles, Cpu } from "lucide-react";
import MahflowTextLogo from "./icons/MahflowTextLogo";
import MahflowIcon from "./icons/MahflowIcon";
import { useSettings } from "../hooks/useSettings";
import {
  GeneralSettings,
  AdvancedSettings,
  HistorySettings,
  DebugSettings,
  AboutSettings,
  PostProcessingSettings,
  ModelsSettings,
} from "./settings";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType;
  enabled: (settings: any) => boolean;
}

export const SECTIONS_CONFIG = {
  general: {
    labelKey: "sidebar.general",
    icon: MahflowIcon,
    component: GeneralSettings,
    enabled: () => true,
  },
  models: {
    labelKey: "sidebar.models",
    icon: Cpu,
    component: ModelsSettings,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Cog,
    component: AdvancedSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: History,
    component: HistorySettings,
    enabled: () => true,
  },
  postprocessing: {
    labelKey: "sidebar.postProcessing",
    icon: Sparkles,
    component: PostProcessingSettings,
    enabled: (settings) => settings?.post_process_enabled ?? false,
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: FlaskConical,
    component: DebugSettings,
    enabled: (settings) => settings?.debug_mode ?? false,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
    component: AboutSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(([_, config]) => config.enabled(settings))
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <div className="glass-panel flex flex-col w-40 h-full border-e border-mid-gray/15 items-center px-2">
      <MahflowTextLogo width={120} className="m-4" />
      <div className="flex flex-col w-full items-center gap-1 pt-2 border-t border-mid-gray/15">
        {availableSections.map((section) => {
          const Icon = section.icon;
          const isActive = activeSection === section.id;

          return (
            <button
              key={section.id}
              type="button"
              aria-current={isActive ? "page" : undefined}
              title={t(section.labelKey)}
              className={`group relative flex gap-2 items-center p-2 w-full rounded-xl cursor-pointer text-start transition-all duration-200 ease-out focus:outline-none focus-visible:ring-2 focus-visible:ring-background-ui/60 ${
                isActive
                  ? "bg-background-ui text-white shadow-[0_6px_18px_rgba(79,100,216,0.4)]"
                  : "text-text/70 hover:text-text hover:bg-white/40 dark:hover:bg-white/5"
              }`}
              onClick={() => onSectionChange(section.id)}
            >
              <span
                aria-hidden="true"
                className={`absolute inset-y-1.5 start-0 w-[3px] rounded-full bg-white/90 transition-opacity duration-200 ${
                  isActive ? "opacity-100" : "opacity-0"
                }`}
              />
              <Icon
                width={20}
                height={20}
                className={`shrink-0 transition-colors ${
                  isActive ? "opacity-100" : "opacity-70 group-hover:opacity-100"
                }`}
              />
              <p className="text-sm font-medium truncate">
                {t(section.labelKey)}
              </p>
            </button>
          );
        })}
      </div>
    </div>
  );
};
