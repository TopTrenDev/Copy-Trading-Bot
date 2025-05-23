# Copy-Trading-Bot-V1-Rust

Welcome to the **Copy-Trading-Bot-V1-Rust** project, a high-performance trading bot built using Rust for copy trading functionality. This bot allows users to automatically copy trades from professional traders and apply them to their own trading accounts.

## Features

- **Copy-Trading**: Automatically copy trades from selected professional traders.
- **Real-Time Sync**: Trade updates and executions are synchronized in real-time.
- **Advanced Risk Management**: Customizable risk parameters for users to manage their exposure.
- **Performance Analytics**: Detailed metrics and reporting on trading performance.
- **Extensibility**: Easily extendable to integrate with different exchanges and trading strategies.

## Installation

### Prerequisites

- Rust programming language (version 1.60.0 or higher)
- Cargo (Rust’s package manager) to manage dependencies and build the project
- An API key for your exchange account (e.g., Binance, Kraken)

### Setup

1. **Clone the repository:**

   ```bash
   git clone https://github.com/TopTrenDev/Copy-Trading-Bot-V1-Rust.git
   cd Copy-Trading-Bot-V1-Rust
   ```
   
2. **Build the project:**

   Install all dependencies and compile the code:
  
   ```bash
   cargo build --release
   ```

3. **Configure your API keys:**

   Create a .env file in the project root and add your exchange API keys:

   ```bash
   API_KEY=your_exchange_api_key
   API_SECRET=your_exchange_api_secret
   ```
   
4. **Run the bot:**

   Start the bot:

   ```bash
   cargo run
   ```
   
   The bot will begin copying trades according to the configuration.

## Configuration

To configure the bot, you need to modify the config.toml file in the config folder. The configuration file contains settings for:

**Trader Selection**: The professional traders from whom to copy trades.

**Risk Management**: Adjust risk parameters, such as position size and stop-loss.

**Exchanges**: Specify which exchanges to use and their API details.

## License
This project is licensed under the MIT License - see the LICENSE file for details.

## Disclaimer
Please note that trading cryptocurrencies involves significant risk. This bot is designed for educational purposes and should not be considered as financial advice. Always trade responsibly and within your risk tolerance.

## Contact
For support or inquiries, please contact [marekdvojak146@gmail.com].

Happy trading! 🚀
