import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { type Theme } from "@/bindings";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";

interface ThemeSelectorProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const ThemeSelector: React.FC<ThemeSelectorProps> = ({
  descriptionMode = "inline",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();

  const options = [
    { value: "system", label: t("settings.appearance.theme.options.system") },
    { value: "light", label: t("settings.appearance.theme.options.light") },
    { value: "dark", label: t("settings.appearance.theme.options.dark") },
    { value: "mah", label: t("settings.appearance.theme.options.mah") },
  ];

  const currentValue = (getSetting("theme") as Theme | undefined) ?? "system";

  return (
    <SettingContainer
      title={t("settings.appearance.theme.title")}
      description={t("settings.appearance.theme.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
    >
      <Dropdown
        options={options}
        selectedValue={currentValue}
        onSelect={(value) => updateSetting("theme", value as Theme)}
      />
    </SettingContainer>
  );
};
