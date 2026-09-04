library ieee;
use ieee.std_logic_1164.all;
entity pipeline_stage is
generic(DATA_WIDTH: positive := 32; REGISTER_OUTPUT: boolean := true);
port(clk: in std_logic; reset_n: in std_logic;

valid_in: in std_logic; data_in: in std_logic_vector(DATA_WIDTH - 1 downto 0);
valid_out: out std_logic; data_out: out std_logic_vector(DATA_WIDTH - 1 downto 0));
end entity;
architecture rtl of pipeline_stage is
type state_t is (idle, active);
signal current_state: state_t := idle;
signal next_state: state_t := idle;

-- Payload registers
signal payload: std_logic_vector(DATA_WIDTH - 1 downto 0);
signal payload_valid: std_logic := '0';
begin
registers: process(clk)
begin
if rising_edge(clk) then
if reset_n = '0' then
current_state <= idle;
payload_valid <= '0';
else
current_state <= next_state;
payload_valid <= valid_in;
if valid_in = '1' then
payload <= data_in;
end if;
end if;
end if;
end process;
control: process(all)
begin
next_state <= current_state;
case current_state is
when idle =>
if valid_in = '1' then next_state <= active; end if;
when active =>
if valid_in = '0' then next_state <= idle; end if;
end case;
end process;
output_buffer: entity work.output_buffer
generic map(WIDTH => DATA_WIDTH, REGISTERED => REGISTER_OUTPUT)
port map(clk => clk, reset_n => reset_n,

valid_in => payload_valid, data_in => payload,
-- External outputs
valid_out => valid_out, data_out => data_out);
end architecture;
