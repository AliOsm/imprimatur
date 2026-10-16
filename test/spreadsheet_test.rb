# frozen_string_literal: true

require_relative 'test_helper'

class SpreadsheetTest < Minitest::Test
  def workbook
    Imprimatur.xlsx do |book|
      book.sheet('Suppliers') do |sheet|
        sheet.row 'Instructions: one supplier a row'
        sheet.row nil
        sheet.row " Company Name\t", 'Country', 'Since', 'Share', 'Price', 'Active', 'Total'
        sheet.row 'Acme', 'PL', Date.new(2024, 3, 15), 0.5, 2, true, Imprimatur.formula('E4*2')
        sheet.write(3, 4, 2, style: Imprimatur.style(number_format: '0.00'))
        sheet.write(3, 3, 0.5, style: Imprimatur.style(number_format: '0%'))
        sheet.row 'Bolt', nil, Time.utc(2024, 3, 15, 9, 30), 1, 1_250, false
      end
      book.sheet('Countries') { it.row 'Name' }
    end
  end

  def test_reads_typed_rows
    book = Imprimatur::Spreadsheet.open(workbook)
    acme, bolt = book.sheet.rows.values_at(3, 4)

    assert_equal ['Acme', 'PL', Date.new(2024, 3, 15), 0.5, 2.0, true, nil], acme
    assert_instance_of Float, acme[4], 'a whole number shown as 2.00'
    assert_equal ['Bolt', nil, DateTime.new(2024, 3, 15, 9, 30), 1, 1250, false], bolt
    assert_instance_of Integer, bolt[4]
    assert_empty book.sheet.rows[1], 'an empty row keeps its place'
    assert_raises(KeyError) { book.sheet('Missing') }
  end

  def test_parses_rows_under_the_header_row
    sheet = Imprimatur.spreadsheet(workbook).sheet
    rows = sheet.parse(clean: true, name: 'Company Name', since: 'Since')

    assert_equal [{ name: 'Acme', since: Date.new(2024, 3, 15) },
                  { name: 'Bolt', since: DateTime.new(2024, 3, 15, 9, 30) }], rows

    error = assert_raises(Imprimatur::HeaderRowNotFound) do
      sheet.parse(name: 'Company Name', country: 'Country', vat: 'VAT', city: 'City')
    end

    assert_equal %w[Company\ Name VAT City], error.missing, 'unclean, Company Name has a tab'
    assert_equal 'no header row has Company Name, VAT, City', error.message
    assert_raises(Imprimatur::HeaderRowNotFound) { sheet.parse(header_rows: 2, country: 'Country') }
  end

  def test_refuses_what_is_no_workbook
    pdf = Imprimatur.pdf { it.paragraph 'Not a sheet.' }
    error = assert_raises(Imprimatur::UnsupportedDocument) { Imprimatur::Spreadsheet.open(pdf) }

    assert_match(/a PDF is no spreadsheet/, error.message)
  end
end
