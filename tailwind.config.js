/** @type {import('tailwindcss').Config} */
export default {
  content: ["./src/**/*.{html,js}"],
  theme: {
    extend: {
      colors: {
        ink: "#070b10",
        mist: "#93a4b8",
        cyanedge: "#3dd9c6",
        ember: "#f59f4c",
      },
      boxShadow: {
        panel: "0 32px 80px rgba(3, 7, 18, 0.55)",
      },
    },
  },
  plugins: [],
};

