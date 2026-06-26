import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { type TranscribeActivation } from "@/bindings";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";

interface TranscribeActivationSelectorProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const TranscribeActivationSelector: React.FC<
  TranscribeActivationSelectorProps
> = ({ descriptionMode = "tooltip", grouped = false }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const options = [
    {
      value: "single_press",
      label: t("settings.general.transcribeActivation.options.singlePress"),
    },
    {
      value: "double_press",
      label: t("settings.general.transcribeActivation.options.doublePress"),
    },
  ];

  const currentValue =
    (getSetting("transcribe_activation") as TranscribeActivation | undefined) ??
    "single_press";

  return (
    <SettingContainer
      title={t("settings.general.transcribeActivation.title")}
      description={t("settings.general.transcribeActivation.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
    >
      <Dropdown
        options={options}
        selectedValue={currentValue}
        onSelect={(value) =>
          updateSetting("transcribe_activation", value as TranscribeActivation)
        }
        disabled={isUpdating("transcribe_activation")}
      />
    </SettingContainer>
  );
};
