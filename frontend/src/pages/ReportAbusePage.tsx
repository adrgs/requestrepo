import { ReportAbuseDialog } from "@/components/abuse/ReportAbuseDialog";
import { StandaloneLayout } from "@/components/layout/StandaloneLayout";
import { usePageTitle } from "@/hooks/usePageTitle";

/**
 * /abuse, for people who are not requestrepo users: the report dialog on its own. It lives
 * outside the dashboard, which would create a session and open a WebSocket for them.
 */
export function ReportAbusePage() {
  usePageTitle("Report abuse");
  const initial =
    new URLSearchParams(window.location.search).get("subdomain") ?? "";

  return (
    <StandaloneLayout>
      <p className="mt-auto text-center text-xs text-default-400">
        Reporting from a script? See the{" "}
        <a className="text-primary hover:underline" href="/docs/abuse.md">
          reporting API
        </a>
        .
      </p>
      <ReportAbuseDialog
        isOpen
        isDismissable={false}
        initialSubdomain={initial}
        onClose={() => {}}
      />
    </StandaloneLayout>
  );
}
