import { ReactFlowProvider } from "@xyflow/react";
import LitApp from "./components/litgraph/LitApp";

function App() {
  return (
    <ReactFlowProvider>
      <LitApp />
    </ReactFlowProvider>
  );
}

export default App;
