entity layout is
    generic ( short: natural );
    port (
        this_identifier_is_deliberately_long_enough_to_force_the_interface_group_to_break:
            in std_logic_vector(255 downto 0)
    );
end entity;

architecture rtl of layout is
begin
    child: entity work.layout
        generic map ( short => 1 );
end architecture;
