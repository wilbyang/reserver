# Booking System

This project is a booking system implemented in Rust using PostgreSQL. It allows users to book or reserve resources such as hotel rooms, meeting rooms, devices, etc. The system uses PostgreSQL's `EXCLUDE` constraint and `RANGE` feature to ensure that resources are not double-booked.

## Features

- **Resource Management**: Manage resources that can be booked.
- **Booking Management**: Create bookings for resources with time constraints.
- **Conflict Prevention**: Uses PostgreSQL's `EXCLUDE` constraint to prevent overlapping bookings.

## Prerequisites

- Rust (latest stable version recommended)
- PostgreSQL
- Cargo (Rust package manager)

## Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/yourusername/booking-system.git
   cd booking-system
   ```

2. **Set up the database**:
   - Create a PostgreSQL database named `booking_system`.
   - Run the following SQL commands to set up the tables:

     ```sql
     CREATE TABLE resources (
         id SERIAL PRIMARY KEY,
         name TEXT NOT NULL
     );

     CREATE TABLE bookings (
         id SERIAL PRIMARY KEY,
         resource_id INT NOT NULL REFERENCES resources(id),
         timespan TSTZRANGE NOT NULL,
         note TEXT,
         user_id VARCHAR(64) NOT NULL,
         CONSTRAINT bookings_conflict EXCLUDE USING gist (resource_id WITH =, timespan WITH &&)
     );
     ```

3. **Configure the database URL**:
   - Update the `database_url` in `src/main.rs` with your PostgreSQL credentials.

4. **Build and run the project**:
   ```bash
   cargo build
   cargo run
   ```

## Usage

- The system will attempt to create a booking for a resource. If the booking conflicts with an existing one, it will fail due to the `EXCLUDE` constraint.

## Dependencies

- [tokio](https://crates.io/crates/tokio) - Asynchronous runtime for Rust.
- [sqlx](https://crates.io/crates/sqlx) - Async SQL toolkit for Rust.
- [chrono](https://crates.io/crates/chrono) - Date and time library for Rust.

## Contributing

Contributions are welcome! Please fork the repository and submit a pull request for any improvements or bug fixes.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Contact

For any questions or issues, please open an issue on GitHub or contact the project maintainer at [your-email@example.com]. 