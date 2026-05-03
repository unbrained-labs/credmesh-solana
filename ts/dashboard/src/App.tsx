import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { SolanaProviders } from './providers/SolanaProviders';
import { Layout } from './components/Layout';
import { LpView } from './views/LpView';
import { AgentView } from './views/AgentView';
import { GovernanceView } from './views/GovernanceView';

export default function App() {
  return (
    <SolanaProviders>
      <BrowserRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<LpView />} />
            <Route path="/agent" element={<AgentView />} />
            <Route path="/governance" element={<GovernanceView />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </SolanaProviders>
  );
}
