import type { DetailedHTMLProps, HTMLAttributes } from "react";

declare module "react" {
  namespace JSX {
    interface IntrinsicElements {
      "lord-icon": DetailedHTMLProps<
        HTMLAttributes<HTMLElement> & {
          src?: string;
          trigger?: string;
          state?: string;
          colors?: string;
          stroke?: string;
        },
        HTMLElement
      >;
    }
  }
}

export {};
