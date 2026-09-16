// Market settings: the plugins page's default browsing source and the cache
// refresh interval. The refresh interval feeds the harness market cache TTL
// from the next launch on.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { usePreferencesStore } from "@/app/store/preferences";
import { useBlocking } from "@/app/store/busy";
import { Card } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";

// 1 minute .. 7 days, mirrored by the backend's bound check.
const MIN_REFRESH_MINUTES = 1;
const MAX_REFRESH_MINUTES = 7 * 24 * 60;

export function MarketSection() {
  const { t } = useTranslation();

  const marketDefaultSource = usePreferencesStore((s) => s.marketDefaultSource);
  const marketRefreshIntervalSeconds = usePreferencesStore((s) => s.marketRefreshIntervalSeconds);
  const setMarketDefaultSource = usePreferencesStore((s) => s.setMarketDefaultSource);
  const setMarketRefreshInterval = usePreferencesStore((s) => s.setMarketRefreshInterval);
  const busy = useBlocking();

  // The minutes draft reseeds whenever the persisted value changes, adjusted
  // during render (per React's recommended pattern for state derived from
  // props) so no effect is needed.
  const [prevSeconds, setPrevSeconds] = useState(marketRefreshIntervalSeconds);
  const [minutesDraft, setMinutesDraft] = useState(
    String(Math.round(marketRefreshIntervalSeconds / 60)),
  );
  if (prevSeconds !== marketRefreshIntervalSeconds) {
    setPrevSeconds(marketRefreshIntervalSeconds);
    setMinutesDraft(String(Math.round(marketRefreshIntervalSeconds / 60)));
  }

  const minutesValue = Number(minutesDraft);
  const minutesValid =
    Number.isInteger(minutesValue) &&
    minutesValue >= MIN_REFRESH_MINUTES &&
    minutesValue <= MAX_REFRESH_MINUTES;
  const minutesDirty =
    minutesValid && minutesValue !== Math.round(marketRefreshIntervalSeconds / 60);

  const digitsOnly = (value: string) => value.replace(/\D/g, "");

  const saveInterval = async () => {
    if (!minutesValid || !minutesDirty) return;
    await setMarketRefreshInterval(minutesValue * 60);
  };

  return (
    <section className="space-y-3">
      <Card>
        <div className="divide-y divide-border">
          <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
            <div>
              <div className="text-heading font-semibold text-text">
                {t("settings.marketDefaultSource")}
              </div>
              <p className="mt-0.5 text-small text-text-dim">
                {t("settings.marketDefaultSourceHint")}
              </p>
            </div>
            <Select
              value={marketDefaultSource}
              onValueChange={(value) => setMarketDefaultSource(value)}
            >
              <SelectTrigger className="w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="curated">{t("plugins.marketSourceCurated")}</SelectItem>
                <SelectItem value="community">{t("plugins.marketSourceCommunity")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
            <div>
              <div className="text-heading font-semibold text-text">
                {t("settings.marketRefreshInterval")}
              </div>
              <p className="mt-0.5 text-small text-text-dim">{t("settings.marketRefreshHint")}</p>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <div className="relative">
                <Input
                  value={minutesDraft}
                  onChange={(event) => setMinutesDraft(digitsOnly(event.target.value))}
                  inputMode="numeric"
                  className="w-[120px] pr-14"
                  aria-label={t("settings.marketRefreshInterval")}
                />
                <span className="absolute right-3 top-1/2 -translate-y-1/2 text-small text-text-faint">
                  {t("settings.marketRefreshUnit")}
                </span>
              </div>
              {minutesDirty && (
                <Button
                  variant="primary"
                  size="sm"
                  onClick={saveInterval}
                  disabled={busy || !minutesValid}
                >
                  {t("settings.save")}
                </Button>
              )}
            </div>
          </div>
        </div>
      </Card>
    </section>
  );
}
