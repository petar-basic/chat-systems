import { createContext, useContext } from 'react';

export const DismissContext = createContext<() => void>(() => {});

export const useDismiss = () => useContext(DismissContext);
