// Source trust badge: official (harness vendor), vetted (third-party,
// reviewed and listed by the DeepMate maintainers), community (raw npm
// search). Unknown or missing signals fall back to community.
import { useTranslation } from "react-i18next";
import { Badge } from "../ui/badge";
import type { MarketTrust } from "../../api";

export function TrustBadge({ trust }: { trust: MarketTrust }) {
  const { t } = useTranslation();
  if (trust === "official") {
    return <Badge variant="accent">{t("plugins.trust.official")}</Badge>;
  }
  if (trust === "vetted") {
    return <Badge variant="accent">{t("plugins.trust.vetted")}</Badge>;
  }
  return <Badge variant="neutral">{t("plugins.trust.community")}</Badge>;
}
