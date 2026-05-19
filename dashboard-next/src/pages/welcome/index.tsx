import { type Component } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { Button } from "@/components/Form";
import { Card, PageHeader } from "@/components/Card";
import { t } from "@/i18n";

const WelcomePage: Component = () => {
  const navigate = useNavigate();
  return (
    <>
      <PageHeader title={t("welcome.title")} subtitle={t("welcome.subtitle")} />
      <Card title={t("login.submit")}>
        <Button variant="primary" onClick={() => navigate("/auth/login")}>
          {t("login.submit")}
        </Button>
      </Card>
    </>
  );
};

export default WelcomePage;
