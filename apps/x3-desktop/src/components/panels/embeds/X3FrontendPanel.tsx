import React from "react";
import IframePanel from "@/components/panels/IframePanel";

const DEV_URL = "http://localhost:4173";

const X3FrontendPanel: React.FC = () => (
  <IframePanel url={DEV_URL} title="X3 Landing & Pages" />
);

export default X3FrontendPanel;
